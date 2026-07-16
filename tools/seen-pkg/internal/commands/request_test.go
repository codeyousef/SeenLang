package commands

import (
	"strings"
	"testing"
)

func TestDecodeRequest(t *testing.T) {
	t.Parallel()
	request := "SEENPKG1\n3\n16\n--expect-version\n6\n0.10.0\n5\nfetch\n"
	arguments, err := DecodeRequest(strings.NewReader(request))
	if err != nil {
		t.Fatal(err)
	}
	want := []string{"--expect-version", "0.10.0", "fetch"}
	for index := range want {
		if arguments[index] != want[index] {
			t.Fatalf("arguments = %#v", arguments)
		}
	}
}

func TestDecodeRequestRejectsMalformedTruncatedAndOversized(t *testing.T) {
	t.Parallel()
	cases := []string{
		"WRONG\n0\n", "SEENPKG1\n01\n", "SEENPKG1\n1\n5\nabc\n",
		"SEENPKG1\n1\n3\nabc", "SEENPKG1\n0\ntrailing", "SEENPKG1\n1\n1048577\n",
		"SEENPKG1\n1\n2\n\xff\xff\n",
	}
	for index, input := range cases {
		if _, err := DecodeRequest(strings.NewReader(input)); err == nil {
			t.Errorf("case %d unexpectedly succeeded", index)
		}
	}
}
