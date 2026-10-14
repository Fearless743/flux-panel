package service

import "sync"

// dedupInt64s 保序去重（批量接口按唯一 ID 处理，避免重复下发/重复计数）
func dedupInt64s(ids []int64) []int64 {
	seen := make(map[int64]struct{}, len(ids))
	out := make([]int64, 0, len(ids))
	for _, id := range ids {
		if _, ok := seen[id]; ok {
			continue
		}
		seen[id] = struct{}{}
		out = append(out, id)
	}
	return out
}

// parallelEach 对 0..n-1 限并发执行 fn，返回按下标顺序收集的非空错误文本。
// fn 返回 "" 表示成功；只读共享数据可安全并发访问。
func parallelEach(n, workers int, fn func(i int) string) []string {
	if n <= 0 {
		return nil
	}
	if workers <= 0 || workers > n {
		workers = n
	}
	type indexedErr struct {
		idx int
		msg string
	}
	jobs := make(chan int, n)
	out := make(chan indexedErr, n)
	var wg sync.WaitGroup
	for w := 0; w < workers; w++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for i := range jobs {
				if msg := fn(i); msg != "" {
					out <- indexedErr{idx: i, msg: msg}
				}
			}
		}()
	}
	for i := 0; i < n; i++ {
		jobs <- i
	}
	close(jobs)
	wg.Wait()
	close(out)
	byIdx := make(map[int]string, len(out))
	for r := range out {
		byIdx[r.idx] = r.msg
	}
	errs := make([]string, 0, len(byIdx))
	for i := 0; i < n; i++ {
		if msg, ok := byIdx[i]; ok {
			errs = append(errs, msg)
		}
	}
	return errs
}

// nodeJob 一次面向单个节点的下发动作。
// 组内（同 nodeID）严格按加入顺序串行，组间（不同 nodeID）限并发——
// 既保证同节点上“先建 chain、再建引用它的 service”的顺序，又让多节点下发重叠 IO 等待。
type nodeJob struct {
	nodeID int64
	run    func() string // 返回 "" 表示成功，否则为失败文案
}

// runNodeJobs 按 nodeID 分组后并行执行，返回按节点首次出现顺序、组内顺序收集的非空失败文案。
func runNodeJobs(jobs []nodeJob) []string {
	if len(jobs) == 0 {
		return nil
	}
	groups := make(map[int64][]func() string)
	order := make([]int64, 0)
	for _, j := range jobs {
		if _, ok := groups[j.nodeID]; !ok {
			order = append(order, j.nodeID)
		}
		groups[j.nodeID] = append(groups[j.nodeID], j.run)
	}

	results := make([][]string, len(order))
	parallelEach(len(order), batchConcurrency, func(i int) string {
		var msgs []string
		for _, fn := range groups[order[i]] {
			if m := fn(); m != "" {
				msgs = append(msgs, m)
			}
		}
		results[i] = msgs
		return ""
	})

	var out []string
	for _, ms := range results {
		out = append(out, ms...)
	}
	return out
}
