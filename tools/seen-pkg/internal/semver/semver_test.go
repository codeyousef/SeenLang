package semver

import (
	"math/rand"
	"sort"
	"testing"
)

func TestExactVersions(t *testing.T) {
	t.Parallel()
	valid := []string{"0.1.0", "1.0.0", "1.0.0-alpha", "1.0.0-alpha.1", "1.0.0-0A-0", "1.0.0+build.01", "1.0.0-alpha+build.5"}
	invalid := []string{"1", "1.2", "01.2.3", "1.02.3", "1.2.03", "1.2.3-", "1.2.3-01", "1.2.3-alpha..1", "1.2.3+", "1.2.3+build..1", "^1.2.3", "~1.2.3", "1.2.*"}
	for _, input := range valid {
		if _, err := Parse(input); err != nil {
			t.Errorf("Parse(%q): %v", input, err)
		}
	}
	for _, input := range invalid {
		if _, err := Parse(input); err == nil {
			t.Errorf("Parse(%q) unexpectedly succeeded", input)
		}
	}
}

func TestRequirements(t *testing.T) {
	t.Parallel()
	valid := []string{"2.1.0", "1.2.3-alpha.1+build.5", "^2.1.0", "^0.2.3", "^0.0.3", "^0.0.0", "^1.2.3-alpha.1", "~1.4", "~1.4.2", "~1.4.2-beta.1", ">=1.2.3 <2.0.0", ">1.2.3 <=1.9.9", ">=1.2.3-alpha.1 <2.0.0"}
	invalid := []string{"", "1.4", "^1.4", "~1", "^1.2.3+build", "~1.2.3+build", ">=1.2.3+build <2.0.0", "^1.2.3 || ^2.0.0", "1.2.*", "1.x", "latest", " ^1.2.3", "^1.2.3 ", ">=1.2.3  <2.0.0", ">=1.2.3\t<2.0.0", ">=1.2.3,<2.0.0", ">=2.0.0 <1.0.0", ">=1.2.3 <1.2.3", "01.2.3", "1.2.3-01", ">=1.2.3", "1.2.3 - 2.0.0", "=1.2.3"}
	for _, input := range valid {
		if _, err := ParseRequirement(input); err != nil {
			t.Errorf("ParseRequirement(%q): %v", input, err)
		}
	}
	for _, input := range invalid {
		if _, err := ParseRequirement(input); err == nil {
			t.Errorf("ParseRequirement(%q) unexpectedly succeeded", input)
		}
	}
}

func TestRangeAndPrereleaseMatching(t *testing.T) {
	t.Parallel()
	requirement, _ := ParseRequirement("^1.2.3-alpha.1")
	for _, input := range []string{"1.2.3-alpha.1", "1.2.3", "1.9.0"} {
		version, _ := Parse(input)
		if !requirement.Matches(version) {
			t.Errorf("%s should match", input)
		}
	}
	other, _ := Parse("1.3.0-beta.1")
	if requirement.Matches(other) {
		t.Fatal("different-core prerelease matched")
	}
	exact, _ := ParseRequirement("1.2.3+one")
	build, _ := Parse("1.2.3+two")
	if exact.Matches(build) {
		t.Fatal("exact requirement ignored build metadata")
	}
}

func TestOrderingIsPermutationIndependent(t *testing.T) {
	t.Parallel()
	inputs := []string{"1.0.0-alpha", "1.0.0-alpha.1", "1.0.0-beta", "1.0.0", "1.2.0", "2.0.0"}
	expected := append([]string(nil), inputs...)
	for seed := int64(0); seed < 50; seed++ {
		got := append([]string(nil), inputs...)
		rand.New(rand.NewSource(seed)).Shuffle(len(got), func(i, j int) { got[i], got[j] = got[j], got[i] })
		sort.Slice(got, func(i, j int) bool {
			left, _ := Parse(got[i])
			right, _ := Parse(got[j])
			return Compare(left, right) < 0
		})
		for index := range expected {
			if got[index] != expected[index] {
				t.Fatalf("seed %d: got %v", seed, got)
			}
		}
	}
}
