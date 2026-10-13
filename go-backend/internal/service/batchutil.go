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
