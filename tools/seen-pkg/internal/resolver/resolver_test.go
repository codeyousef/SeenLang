package resolver

import (
	"context"
	"math/rand"
	"testing"

	"github.com/codeyousef/seen/tools/seen-pkg/internal/model"
)

const origin = "https://seen.dev.yousef.codes/packages"

func digest(c byte) string {
	value := make([]byte, 64)
	for index := range value {
		value[index] = c
	}
	return string(value)
}
func edge(alias, pkg, requirement string, allow ...model.Capability) model.Edge {
	return model.Edge{Alias: alias, Package: pkg, RegistryOrigin: origin, Requirement: requirement, Allow: allow}
}
func candidate(pkg, version string, hash byte, dependencies ...model.Edge) model.Candidate {
	d := digest(hash)
	target, _ := model.TargetPath(pkg, version, d)
	return model.Candidate{Package: pkg, Version: version, RegistryOrigin: origin, ArchiveSHA256: d, TargetPath: target, MetadataVersion: 1, Availability: model.Available, Dependencies: dependencies}
}
func repository(items ...model.Candidate) MemoryRepository {
	result := MemoryRepository{Items: map[model.PackageKey][]model.Candidate{}}
	for _, item := range items {
		result.Items[item.Key()] = append(result.Items[item.Key()], item)
	}
	return result
}

func TestDiamondAndCanonicalBacktracking(t *testing.T) {
	t.Parallel()
	root := model.Root{Name: "app", Version: "1.0.0", Dependencies: []model.Edge{edge("app_dep", "alice/app", "^1.0.0"), edge("core", "seen/core", "^1.0.0")}, Grants: map[string][]model.Capability{}}
	repo := repository(
		candidate("alice/app", "1.2.0", 'a', edge("core", "seen/core", "^2.0.0")),
		candidate("alice/app", "1.1.0", 'b', edge("core", "seen/core", "^1.0.0")),
		candidate("seen/core", "2.1.0", 'c'), candidate("seen/core", "1.9.0", 'd'),
	)
	result, err := Resolve(context.Background(), root, repo, Options{Strategy: Update})
	if err != nil {
		t.Fatal(err)
	}
	if len(result.Packages) != 2 || result.Packages[0].Version != "1.1.0" || result.Packages[1].Version != "1.9.0" {
		t.Fatalf("selected %#v", result.Packages)
	}
}

func TestResolutionIsPermutationIndependent(t *testing.T) {
	t.Parallel()
	rootEdges := []model.Edge{edge("a", "alice/a", "^1.0.0"), edge("b", "bob/b", "^1.0.0")}
	base := []model.Candidate{
		candidate("alice/a", "1.0.0", 'a', edge("util", "seen/util", "^1.0.0")),
		candidate("bob/b", "1.0.0", 'b', edge("util", "seen/util", ">=1.5.0 <2.0.0")),
		candidate("seen/util", "1.8.0", 'c'), candidate("seen/util", "1.6.0", 'd'),
	}
	for seed := int64(0); seed < 50; seed++ {
		rng := rand.New(rand.NewSource(seed))
		edges := append([]model.Edge(nil), rootEdges...)
		candidates := append([]model.Candidate(nil), base...)
		rng.Shuffle(len(edges), func(i, j int) { edges[i], edges[j] = edges[j], edges[i] })
		rng.Shuffle(len(candidates), func(i, j int) { candidates[i], candidates[j] = candidates[j], candidates[i] })
		result, err := Resolve(context.Background(), model.Root{Name: "root", Version: "1.0.0", Dependencies: edges, Grants: map[string][]model.Capability{}}, repository(candidates...), Options{Strategy: Update})
		if err != nil {
			t.Fatalf("seed %d: %v", seed, err)
		}
		if len(result.Packages) != 3 || result.Packages[2].Version != "1.8.0" {
			t.Fatalf("seed %d: %#v", seed, result.Packages)
		}
	}
}

func TestStableConflictRequesters(t *testing.T) {
	t.Parallel()
	root := model.Root{Name: "root", Version: "1.0.0", Dependencies: []model.Edge{edge("a", "alice/a", "1.0.0"), edge("b", "bob/b", "1.0.0")}, Grants: map[string][]model.Capability{}}
	repo := repository(
		candidate("alice/a", "1.0.0", 'a', edge("util", "seen/util", "^1.0.0")),
		candidate("bob/b", "1.0.0", 'b', edge("util", "seen/util", "^2.0.0")),
		candidate("seen/util", "1.9.0", 'c'), candidate("seen/util", "2.1.0", 'd'),
	)
	_, err := Resolve(context.Background(), root, repo, Options{Strategy: Update})
	resolutionError, ok := err.(*Error)
	if !ok {
		t.Fatalf("error = %T %v", err, err)
	}
	want := []string{"alice/a@1.0.0", "bob/b@1.0.0"}
	if resolutionError.Code != "dependency_constraint_conflict" || len(resolutionError.ConflictRequesters) != 2 || resolutionError.ConflictRequesters[0] != want[0] || resolutionError.ConflictRequesters[1] != want[1] {
		t.Fatalf("error = %#v", resolutionError)
	}
}

func TestCapabilityConsentOnDirectAndTransitiveNodes(t *testing.T) {
	t.Parallel()
	tls := candidate("seen/tls", "1.0.0", 'b')
	tls.Capabilities = []model.Capability{model.CapabilityNetwork}
	web := candidate("alice/web", "1.0.0", 'a', edge("tls", "seen/tls", "1.0.0", model.CapabilityNetwork))
	root := model.Root{Name: "root", Version: "1.0.0", Dependencies: []model.Edge{edge("web", "alice/web", "1.0.0")}, Grants: map[string][]model.Capability{}}
	_, err := Resolve(context.Background(), root, repository(web, tls), Options{})
	if got := err.(*Error).Code; got != "capability_consent_required" {
		t.Fatalf("code = %s", got)
	}
	root.Grants["seen/tls"] = []model.Capability{model.CapabilityNetwork}
	if _, err := Resolve(context.Background(), root, repository(web, tls), Options{}); err != nil {
		t.Fatal(err)
	}
	root.Dependencies = []model.Edge{edge("tls", "seen/tls", "1.0.0")}
	if _, err := Resolve(context.Background(), root, repository(tls), Options{}); err == nil || err.(*Error).Code != "dependency_capability_not_allowed" {
		t.Fatalf("expected edge policy error, got %v", err)
	}
}

func TestOriginPinningBuildTieAndEquivocationFailClosed(t *testing.T) {
	t.Parallel()
	root := model.Root{Name: "root", Version: "1.0.0", Dependencies: []model.Edge{edge("json", "seen/json", "^1.0.0")}, Grants: map[string][]model.Capability{}}
	foreign := candidate("seen/json", "1.0.0", 'a')
	foreign.RegistryOrigin = "https://seen.yousef.codes/packages"
	repo := MemoryRepository{Items: map[model.PackageKey][]model.Candidate{{RegistryOrigin: origin, Package: "seen/json"}: {foreign}}}
	if _, err := Resolve(context.Background(), root, repo, Options{}); err == nil || err.(*Error).Code != "registry_origin_mismatch" {
		t.Fatalf("origin error = %v", err)
	}
	one, two := candidate("seen/json", "1.5.0+one", 'a'), candidate("seen/json", "1.5.0+two", 'b')
	if _, err := Resolve(context.Background(), root, repository(one, two), Options{}); err == nil || err.(*Error).Code != "ambiguous_build_metadata" {
		t.Fatalf("tie error = %v", err)
	}
	one, two = candidate("seen/json", "1.5.0", 'a'), candidate("seen/json", "1.5.0", 'b')
	if _, err := Resolve(context.Background(), root, repository(one, two), Options{}); err == nil || err.(*Error).Code != "metadata_equivocation" {
		t.Fatalf("equivocation error = %v", err)
	}
}

func TestLockedOfflineAndYankedPolicies(t *testing.T) {
	t.Parallel()
	item := candidate("seen/json", "1.5.0", 'a')
	item.Availability = model.Yanked
	root := model.Root{Name: "root", Version: "1.0.0", Dependencies: []model.Edge{edge("json", "seen/json", "^1.0.0")}, Grants: map[string][]model.Capability{}}
	locked := model.LockedPackage{Package: item.Package, Version: item.Version, Source: "hosted-registry", RegistryOrigin: origin, ArchiveSHA256: item.ArchiveSHA256, TargetPath: item.TargetPath, MetadataVersion: 1}
	lock := &model.Lock{Version: 2, Root: root, Packages: []model.LockedPackage{locked}}
	result, err := Resolve(context.Background(), root, repository(item), Options{Lock: lock})
	if err != nil || result.Packages[0].Version != "1.5.0" {
		t.Fatalf("normal lock = %#v %v", result, err)
	}
	if _, err := Resolve(context.Background(), root, repository(item), Options{Strategy: Update, Lock: lock}); err == nil || err.(*Error).Code != "no_matching_version" {
		t.Fatalf("update = %v", err)
	}
	missing := repository(item)
	missing.OfflineMissing = map[model.PackageKey]bool{item.Key(): true}
	if _, err := Resolve(context.Background(), root, missing, Options{Locked: true, Offline: true, Lock: lock}); err == nil || err.(*Error).Code != "offline_data_unavailable" {
		t.Fatalf("offline = %v", err)
	}
	if _, err := Resolve(context.Background(), root, repository(item), Options{Strategy: Update, Locked: true, Lock: lock}); err == nil || err.(*Error).Code != "invalid_mode_combination" {
		t.Fatalf("mode = %v", err)
	}
}
