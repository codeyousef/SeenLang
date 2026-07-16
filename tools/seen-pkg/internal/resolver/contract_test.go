package resolver

import (
	"context"
	"encoding/json"
	"os"
	"path/filepath"
	"sort"
	"testing"

	"github.com/codeyousef/seen/tools/seen-pkg/internal/model"
)

type graphFixture struct {
	Cases []graphCase `json:"cases"`
}
type graphCase struct {
	Name      string   `json:"name"`
	Strategy  string   `json:"strategy"`
	Modifiers []string `json:"modifiers"`
	Root      struct {
		Dependencies []graphEdge  `json:"dependencies"`
		Grants       []graphGrant `json:"grants"`
	} `json:"root"`
	Candidates []graphCandidate `json:"candidates"`
	Expected   struct {
		Outcome            string           `json:"outcome"`
		ErrorCode          string           `json:"error_code"`
		Selected           []graphSelection `json:"selected"`
		ConflictRequesters []string         `json:"conflict_requesters"`
	} `json:"expected"`
}
type graphSelection struct {
	Package       string `json:"package"`
	Version       string `json:"version"`
	ArchiveSHA256 string `json:"archive_sha256"`
}
type graphEdge struct {
	Alias, Package, RegistryOrigin, Requirement string
	Allow                                       []string
}

func (edge *graphEdge) UnmarshalJSON(data []byte) error {
	var raw struct {
		Alias          string   `json:"alias"`
		Package        string   `json:"package"`
		RegistryOrigin string   `json:"registry_origin"`
		Requirement    string   `json:"requirement"`
		Allow          []string `json:"allow"`
	}
	if err := json.Unmarshal(data, &raw); err != nil {
		return err
	}
	edge.Alias, edge.Package, edge.RegistryOrigin, edge.Requirement, edge.Allow = raw.Alias, raw.Package, raw.RegistryOrigin, raw.Requirement, raw.Allow
	return nil
}

type graphGrant struct {
	Package      string   `json:"package"`
	Capabilities []string `json:"capabilities"`
}
type graphCandidate struct {
	Package        string      `json:"package"`
	Version        string      `json:"version"`
	RegistryOrigin string      `json:"registry_origin"`
	ArchiveSHA256  string      `json:"archive_sha256"`
	Availability   string      `json:"availability"`
	Capabilities   []string    `json:"capabilities"`
	Dependencies   []graphEdge `json:"dependencies"`
}

func TestFrozenGraphContract(t *testing.T) {
	t.Parallel()
	filename := filepath.Join("..", "..", "..", "..", "contracts", "package-registry", "v1", "fixtures", "resolver-graph-cases-v1.json")
	content, err := os.ReadFile(filename)
	if err != nil {
		t.Fatal(err)
	}
	var fixture graphFixture
	if err := json.Unmarshal(content, &fixture); err != nil {
		t.Fatal(err)
	}
	if len(fixture.Cases) == 0 {
		t.Fatal("graph fixture has no cases")
	}
	for _, testCase := range fixture.Cases {
		testCase := testCase
		t.Run(testCase.Name, func(t *testing.T) {
			t.Parallel()
			for permutation := 0; permutation < 2; permutation++ {
				root := model.Root{Name: "fixture_root", Version: "1.0.0", Grants: map[string][]model.Capability{}}
				for _, item := range testCase.Root.Dependencies {
					root.Dependencies = append(root.Dependencies, fixtureEdge(item))
				}
				for _, grant := range testCase.Root.Grants {
					for _, capability := range grant.Capabilities {
						root.Grants[grant.Package] = append(root.Grants[grant.Package], model.Capability(capability))
					}
				}
				items := append([]graphCandidate(nil), testCase.Candidates...)
				if permutation == 1 {
					reverseGraphEdges(root.Dependencies)
					for left, right := 0, len(items)-1; left < right; left, right = left+1, right-1 {
						items[left], items[right] = items[right], items[left]
					}
				}
				var candidates []model.Candidate
				for _, item := range items {
					candidate := model.Candidate{Package: item.Package, Version: item.Version, RegistryOrigin: item.RegistryOrigin, ArchiveSHA256: item.ArchiveSHA256, Availability: model.Availability(item.Availability), MetadataVersion: 1}
					candidate.TargetPath, _ = model.TargetPath(item.Package, item.Version, item.ArchiveSHA256)
					for _, capability := range item.Capabilities {
						candidate.Capabilities = append(candidate.Capabilities, model.Capability(capability))
					}
					for _, dependency := range item.Dependencies {
						candidate.Dependencies = append(candidate.Dependencies, fixtureEdge(dependency))
					}
					candidates = append(candidates, candidate)
				}
				strategy := Normal
				if testCase.Strategy == "update" {
					strategy = Update
				}
				result, resolveErr := Resolve(context.Background(), root, repository(candidates...), Options{Strategy: strategy})
				if testCase.Expected.Outcome == "error" {
					typed, ok := resolveErr.(*Error)
					if !ok || typed.Code != testCase.Expected.ErrorCode {
						t.Fatalf("permutation %d error=%T %v, want %s", permutation, resolveErr, resolveErr, testCase.Expected.ErrorCode)
					}
					if len(testCase.Expected.ConflictRequesters) > 0 {
						if !equalText(typed.ConflictRequesters, testCase.Expected.ConflictRequesters) {
							t.Fatalf("requesters=%v want=%v", typed.ConflictRequesters, testCase.Expected.ConflictRequesters)
						}
					}
					continue
				}
				if resolveErr != nil {
					t.Fatalf("permutation %d: %v", permutation, resolveErr)
				}
				got := make([]string, len(result.Packages))
				for index, pkg := range result.Packages {
					got[index] = pkg.Package + "@" + pkg.Version + "#" + pkg.ArchiveSHA256
				}
				want := make([]string, len(testCase.Expected.Selected))
				for index, pkg := range testCase.Expected.Selected {
					want[index] = pkg.Package + "@" + pkg.Version + "#" + pkg.ArchiveSHA256
				}
				sort.Strings(want)
				if !equalText(got, want) {
					t.Fatalf("permutation %d selected=%v want=%v", permutation, got, want)
				}
			}
		})
	}
}

func fixtureEdge(item graphEdge) model.Edge {
	allow := make([]model.Capability, len(item.Allow))
	for index, value := range item.Allow {
		allow[index] = model.Capability(value)
	}
	return model.Edge{Alias: item.Alias, Package: item.Package, RegistryOrigin: item.RegistryOrigin, Requirement: item.Requirement, Allow: allow}
}
func reverseGraphEdges(values []model.Edge) {
	for left, right := 0, len(values)-1; left < right; left, right = left+1, right-1 {
		values[left], values[right] = values[right], values[left]
	}
}
func equalText(left, right []string) bool {
	if len(left) != len(right) {
		return false
	}
	for index := range left {
		if left[index] != right[index] {
			return false
		}
	}
	return true
}
