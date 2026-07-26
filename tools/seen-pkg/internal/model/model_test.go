package model

import "testing"

func TestCanonicalIdentifiersAndOrigins(t *testing.T) {
	t.Parallel()
	for _, value := range []string{"alice/mathx", "seen/json", "a/b"} {
		if err := ValidateIdentity(value); err != nil {
			t.Errorf("ValidateIdentity(%q): %v", value, err)
		}
	}
	for _, value := range []string{"Alice/mathx", "alice/-math", "alice/math_kit", " alice/mathx", "a/b/c"} {
		if err := ValidateIdentity(value); err == nil {
			t.Errorf("ValidateIdentity(%q) unexpectedly succeeded", value)
		}
	}
	validOrigin := "https://seen.dev.yousef.codes/packages"
	if err := ValidateRegistryOrigin(validOrigin); err != nil {
		t.Fatalf("valid origin: %v", err)
	}
	for _, value := range []string{
		"http://seen.dev.yousef.codes/packages", "https://SEEN.dev.yousef.codes/packages",
		"https://seen.dev.yousef.codes", "https://seen.dev.yousef.codes/packages/",
		"https://seen.dev.yousef.codes/packages?mirror=x", "https://seen.dev.yousef.codes:443/packages",
	} {
		if err := ValidateRegistryOrigin(value); err == nil {
			t.Errorf("ValidateRegistryOrigin(%q) unexpectedly succeeded", value)
		}
	}
}

func TestCapabilities(t *testing.T) {
	t.Parallel()
	if err := ValidateCapabilities([]Capability{CapabilityNetwork, CapabilityFFI}); err != nil {
		t.Fatal(err)
	}
	if err := ValidateCapabilities([]Capability{CapabilityNetwork, CapabilityNetwork}); err == nil {
		t.Fatal("duplicate capability accepted")
	}
	missing := MissingCapabilities([]Capability{CapabilityNetwork}, []Capability{CapabilityFFI, CapabilityNetwork})
	if len(missing) != 1 || missing[0] != CapabilityFFI {
		t.Fatalf("missing = %#v", missing)
	}
}
