package crypto

import "testing"

func TestAESRoundTrip(t *testing.T) {
	c, err := NewAESCrypto("test-secret-key")
	if err != nil {
		t.Fatal(err)
	}
	plain := `{"hello":"world","n":1}`
	enc, err := c.EncryptString(plain)
	if err != nil {
		t.Fatal(err)
	}
	dec, err := c.DecryptString(enc)
	if err != nil {
		t.Fatal(err)
	}
	if dec != plain {
		t.Fatalf("got %q want %q", dec, plain)
	}
}
