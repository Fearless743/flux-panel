package memtune

import "testing"

func TestParseByteSize(t *testing.T) {
	cases := []struct {
		in   string
		want int64
		ok   bool
	}{
		{"", 0, false},
		{"   ", 0, false},
		{"1024", 1024, true},
		{" 1024 ", 1024, true},
		{"512MiB", 512 << 20, true},
		{"512M", 512 << 20, true},
		{"256 KiB", 256 << 10, true},
		{"1GiB", 1 << 30, true},
		{"1G", 1 << 30, true},
		{"1.5GB", 1500000000, true},
		{"0", 0, false},
		{"-1M", 0, false},
		{"abc", 0, false},
		{"12x", 0, false},
		{"max", 0, false}, // cgroup v2 无限制标记
	}

	for _, c := range cases {
		got, ok := ParseByteSize(c.in)
		if ok != c.ok || (ok && got != c.want) {
			t.Errorf("ParseByteSize(%q) = (%d, %v), want (%d, %v)", c.in, got, ok, c.want, c.ok)
		}
	}
}

func TestFormatByteSize(t *testing.T) {
	cases := []struct {
		in   int64
		want string
	}{
		{512, "512B"},
		{256 << 20, "256MiB"},
		{1 << 30, "1.0GiB"},
	}
	for _, c := range cases {
		if got := FormatByteSize(c.in); got != c.want {
			t.Errorf("FormatByteSize(%d) = %q, want %q", c.in, got, c.want)
		}
	}
}

// DetectCgroupMemoryLimit 在容器内应返回正值，在无限制环境返回 0，均不得 panic。
func TestDetectCgroupMemoryLimit(t *testing.T) {
	if v := DetectCgroupMemoryLimit(); v < 0 {
		t.Fatalf("DetectCgroupMemoryLimit() = %d, want >= 0", v)
	}
}

// Tune 在任何环境下都应静默生效（有/无 cgroup 限制），不 panic。
func TestTune(t *testing.T) {
	Tune()
	Tune() // 重复调用安全
}
