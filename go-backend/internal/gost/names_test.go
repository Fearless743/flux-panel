package gost

import "testing"

func TestIsOKSemantics(t *testing.T) {
	if !IsOK("OK") {
		t.Fatal("OK should be success")
	}
	if !IsOK("service x already exists") {
		t.Fatal("exists should be success")
	}
	if IsOK("service x not found") {
		t.Fatal("not found must NOT be success (empty node sync)")
	}
	if !IsMissing("service x not found") {
		t.Fatal("IsMissing")
	}
	if NormalizeOK("service x not found") == "OK" {
		t.Fatal("NormalizeOK must not swallow not found")
	}
	if NormalizeOK("already exists") != "OK" {
		t.Fatal("NormalizeOK exists")
	}
}
