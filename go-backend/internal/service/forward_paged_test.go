package service

import (
	"path/filepath"
	"testing"

	"github.com/Fearless743/flux-panel/go-backend/internal/db"
	"github.com/Fearless743/flux-panel/go-backend/internal/model"
	"github.com/Fearless743/flux-panel/go-backend/internal/repo"
)

func openTestDB(t *testing.T) *ForwardService {
	t.Helper()
	path := filepath.Join(t.TempDir(), "test.db")
	database, err := db.Open(path)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	t.Cleanup(func() { _ = database.Close() })
	// db.Open 已执行 schema（含分页索引）
	return NewForwardService(database, nil)
}

func seedForwards(t *testing.T, s *ForwardService, n int) {
	t.Helper()
	for i := 0; i < n; i++ {
		f := &model.Forward{
			UserID:      1,
			UserName:    "alice",
			Name:        "fw",
			TunnelID:    1,
			RemoteAddr:  "1.1.1.1:80",
			Strategy:    "fifo",
			CreatedTime: int64(1000 + i),
			UpdatedTime: int64(1000 + i),
			Status:      1,
		}
		if i%2 == 1 {
			f.Status = 0
		}
		if _, err := s.Repo.Insert(f); err != nil {
			t.Fatalf("insert: %v", err)
		}
	}
}

func intPtr(v int) *int { return &v }

func TestListPagedBasic(t *testing.T) {
	s := openTestDB(t)
	seedForwards(t, s, 25)

	admin := CurrentUser{UserID: 99, RoleID: 0}
	p1, err := s.ListPaged(admin, ForwardListReq{Page: intPtr(1), PageSize: intPtr(10)})
	if err != nil {
		t.Fatalf("p1: %v", err)
	}
	if p1.Total != 25 || len(p1.List) != 10 {
		t.Fatalf("p1 got total=%d len=%d", p1.Total, len(p1.List))
	}
	p3, err := s.ListPaged(admin, ForwardListReq{Page: intPtr(3), PageSize: intPtr(10)})
	if err != nil {
		t.Fatalf("p3: %v", err)
	}
	if len(p3.List) != 5 {
		t.Fatalf("p3 got len=%d", len(p3.List))
	}
	// 页之间无重叠
	seen := map[int64]bool{}
	for _, f := range append(p1.List, p3.List...) {
		if seen[f.ID] {
			t.Fatalf("dup id %d", f.ID)
		}
		seen[f.ID] = true
	}
}

func TestListPagedFilters(t *testing.T) {
	s := openTestDB(t)
	seedForwards(t, s, 10)

	admin := CurrentUser{UserID: 99, RoleID: 0}
	// status 过滤：偶数下标 status=1 共 5 条
	got, err := s.ListPaged(admin, ForwardListReq{Page: intPtr(1), PageSize: intPtr(20), Status: intPtr(1)})
	if err != nil {
		t.Fatalf("status: %v", err)
	}
	if got.Total != 5 {
		t.Fatalf("status total=%d want 5", got.Total)
	}
	// keyword 匹配 name
	got, err = s.ListPaged(admin, ForwardListReq{Page: intPtr(1), PageSize: intPtr(20), Keyword: "fw"})
	if err != nil {
		t.Fatalf("kw: %v", err)
	}
	if got.Total != 10 {
		t.Fatalf("kw total=%d want 10", got.Total)
	}
	// 不存在的 tunnel
	got, err = s.ListPaged(admin, ForwardListReq{Page: intPtr(1), PageSize: intPtr(20), TunnelID: intPtr(999)})
	if err != nil {
		t.Fatalf("tunnel: %v", err)
	}
	if got.Total != 0 || len(got.List) != 0 {
		t.Fatalf("tunnel got total=%d len=%d", got.Total, len(got.List))
	}
	// 普通用户只能看自己
	user := CurrentUser{UserID: 2, RoleID: 1}
	got, err = s.ListPaged(user, ForwardListReq{Page: intPtr(1), PageSize: intPtr(20)})
	if err != nil {
		t.Fatalf("user: %v", err)
	}
	if got.Total != 0 {
		t.Fatalf("user total=%d want 0", got.Total)
	}
}

func TestListPagedKeywordEscape(t *testing.T) {
	s := openTestDB(t)
	seedForwards(t, s, 3)
	// 含通配符的关键字的 LIKE 不应报错、不应匹配全部
	admin := CurrentUser{UserID: 99, RoleID: 0}
	got, err := s.ListPaged(admin, ForwardListReq{Page: intPtr(1), PageSize: intPtr(20), Keyword: "%_\\"})
	if err != nil {
		t.Fatalf("escape: %v", err)
	}
	if got.Total != 0 {
		t.Fatalf("escape total=%d want 0", got.Total)
	}
}

func TestForwardListReqIsPaged(t *testing.T) {
	if (ForwardListReq{}).IsPaged() {
		t.Fatal("empty should not be paged")
	}
	if !(ForwardListReq{Page: intPtr(1)}).IsPaged() {
		t.Fatal("page should be paged")
	}
	if !(ForwardListReq{Keyword: "x"}).IsPaged() {
		t.Fatal("keyword should be paged")
	}
}

func TestIndexesExist(t *testing.T) {
	s := openTestDB(t)
	var n int
	for _, name := range []string{"idx_forward_user_order", "idx_forward_port_forward", "idx_chain_tunnel_tunnel_type"} {
		if err := s.DB.Get(&n, `SELECT COUNT(1) FROM sqlite_master WHERE type='index' AND name=?`, name); err != nil {
			t.Fatalf("index %s: %v", name, err)
		}
		if n != 1 {
			t.Fatalf("index %s missing", name)
		}
	}
	_ = repo.ForwardListFilter{}
}
