package service

import (
	"sync"
	"testing"
)

// 同 nodeID 必须严格串行，且按加入顺序执行。
func TestRunNodeJobsPerNodeOrder(t *testing.T) {
	var mu sync.Mutex
	order := map[int64][]int{}
	jobs := []nodeJob{
		{nodeID: 1, run: func() string { mu.Lock(); order[1] = append(order[1], 1); mu.Unlock(); return "" }},
		{nodeID: 2, run: func() string { mu.Lock(); order[2] = append(order[2], 1); mu.Unlock(); return "" }},
		{nodeID: 1, run: func() string { mu.Lock(); order[1] = append(order[1], 2); mu.Unlock(); return "" }},
		{nodeID: 2, run: func() string { mu.Lock(); order[2] = append(order[2], 2); mu.Unlock(); return "" }},
	}
	if errs := runNodeJobs(jobs); len(errs) != 0 {
		t.Fatalf("unexpected errs: %v", errs)
	}
	for _, id := range []int64{1, 2} {
		got := order[id]
		if len(got) != 2 || got[0] != 1 || got[1] != 2 {
			t.Fatalf("node %d order = %v, want [1 2]", id, got)
		}
	}
}

// 失败文案按节点首次出现顺序、组内顺序收集。
func TestRunNodeJobsCollectsErrorsInOrder(t *testing.T) {
	jobs := []nodeJob{
		{nodeID: 1, run: func() string { return "e1" }},
		{nodeID: 2, run: func() string { return "" }},
		{nodeID: 3, run: func() string { return "e3" }},
		{nodeID: 1, run: func() string { return "e1b" }},
	}
	errs := runNodeJobs(jobs)
	want := []string{"e1", "e1b", "e3"}
	if len(errs) != len(want) {
		t.Fatalf("errs = %v, want %v", errs, want)
	}
	for i := range want {
		if errs[i] != want[i] {
			t.Fatalf("errs[%d] = %q, want %q", i, errs[i], want[i])
		}
	}
}
