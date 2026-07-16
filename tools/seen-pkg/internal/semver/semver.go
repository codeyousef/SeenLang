// Package semver implements Seen's deliberately small, canonical SemVer v1
// surface. Invalid input is rejected without trimming or repair.
package semver

import (
	"fmt"
	"math"
	"strconv"
	"strings"
)

type Identifier struct {
	Text    string
	Numeric bool
	Number  uint64
}

type Version struct {
	Major      uint64
	Minor      uint64
	Patch      uint64
	Prerelease []Identifier
	Build      []string
	raw        string
}

func (version Version) String() string { return version.raw }
func (version Version) Core() string {
	return strconv.FormatUint(version.Major, 10) + "." + strconv.FormatUint(version.Minor, 10) + "." + strconv.FormatUint(version.Patch, 10)
}
func (version Version) IsPrerelease() bool { return len(version.Prerelease) != 0 }

func Parse(input string) (Version, error) {
	if input == "" || len(input) > 128 || strings.TrimSpace(input) != input {
		return Version{}, fmt.Errorf("invalid canonical SemVer %q", input)
	}
	var version Version
	version.raw = input
	main := input
	if strings.Count(main, "+") > 1 {
		return Version{}, fmt.Errorf("invalid canonical SemVer %q", input)
	}
	if offset := strings.IndexByte(main, '+'); offset >= 0 {
		build := main[offset+1:]
		main = main[:offset]
		if build == "" {
			return Version{}, fmt.Errorf("invalid canonical SemVer %q", input)
		}
		for _, identifier := range strings.Split(build, ".") {
			if !validIdentifier(identifier) {
				return Version{}, fmt.Errorf("invalid canonical SemVer %q", input)
			}
			version.Build = append(version.Build, identifier)
		}
	}
	if strings.Count(main, "-") > 0 {
		if offset := strings.IndexByte(main, '-'); offset >= 0 {
			prerelease := main[offset+1:]
			main = main[:offset]
			if prerelease == "" {
				return Version{}, fmt.Errorf("invalid canonical SemVer %q", input)
			}
			for _, identifier := range strings.Split(prerelease, ".") {
				if !validIdentifier(identifier) {
					return Version{}, fmt.Errorf("invalid canonical SemVer %q", input)
				}
				item := Identifier{Text: identifier, Numeric: allDigits(identifier)}
				if item.Numeric {
					if len(identifier) > 1 && identifier[0] == '0' {
						return Version{}, fmt.Errorf("numeric prerelease identifier has a leading zero in %q", input)
					}
					number, err := strconv.ParseUint(identifier, 10, 64)
					if err != nil {
						return Version{}, fmt.Errorf("numeric prerelease identifier overflows in %q", input)
					}
					item.Number = number
				}
				version.Prerelease = append(version.Prerelease, item)
			}
		}
	}
	parts := strings.Split(main, ".")
	if len(parts) != 3 {
		return Version{}, fmt.Errorf("SemVer %q must contain major.minor.patch", input)
	}
	numbers := []*uint64{&version.Major, &version.Minor, &version.Patch}
	for index, part := range parts {
		if part == "" || !allDigits(part) || (len(part) > 1 && part[0] == '0') {
			return Version{}, fmt.Errorf("invalid canonical SemVer %q", input)
		}
		number, err := strconv.ParseUint(part, 10, 64)
		if err != nil {
			return Version{}, fmt.Errorf("numeric component overflows in %q", input)
		}
		*numbers[index] = number
	}
	return version, nil
}

func allDigits(value string) bool {
	if value == "" {
		return false
	}
	for index := 0; index < len(value); index++ {
		if value[index] < '0' || value[index] > '9' {
			return false
		}
	}
	return true
}

func validIdentifier(value string) bool {
	if value == "" {
		return false
	}
	for index := 0; index < len(value); index++ {
		c := value[index]
		if !(c >= '0' && c <= '9') && !(c >= 'A' && c <= 'Z') && !(c >= 'a' && c <= 'z') && c != '-' {
			return false
		}
	}
	return true
}

// Compare returns -1, 0, or 1 by SemVer precedence. Build metadata is ignored.
func Compare(left, right Version) int {
	for _, pair := range [][2]uint64{{left.Major, right.Major}, {left.Minor, right.Minor}, {left.Patch, right.Patch}} {
		if pair[0] < pair[1] {
			return -1
		}
		if pair[0] > pair[1] {
			return 1
		}
	}
	if len(left.Prerelease) == 0 && len(right.Prerelease) == 0 {
		return 0
	}
	if len(left.Prerelease) == 0 {
		return 1
	}
	if len(right.Prerelease) == 0 {
		return -1
	}
	limit := len(left.Prerelease)
	if len(right.Prerelease) < limit {
		limit = len(right.Prerelease)
	}
	for index := 0; index < limit; index++ {
		l, r := left.Prerelease[index], right.Prerelease[index]
		if l.Numeric && r.Numeric {
			if l.Number < r.Number {
				return -1
			}
			if l.Number > r.Number {
				return 1
			}
			continue
		}
		if l.Numeric != r.Numeric {
			if l.Numeric {
				return -1
			}
			return 1
		}
		if l.Text < r.Text {
			return -1
		}
		if l.Text > r.Text {
			return 1
		}
	}
	if len(left.Prerelease) < len(right.Prerelease) {
		return -1
	}
	if len(left.Prerelease) > len(right.Prerelease) {
		return 1
	}
	return 0
}

type Kind string

const (
	Exact                 Kind = "exact"
	Caret                 Kind = "caret"
	Tilde                 Kind = "tilde"
	ComparatorConjunction Kind = "comparator-conjunction"
)

type Bound struct {
	Version   Version
	Inclusive bool
}

type Requirement struct {
	Raw             string
	Kind            Kind
	Exact           *Version
	Lower           *Bound
	Upper           *Bound
	prereleaseCores map[string]struct{}
}

func (requirement Requirement) String() string { return requirement.Raw }

func ParseRequirement(input string) (Requirement, error) {
	if input == "" || len(input) > 261 || strings.TrimSpace(input) != input || strings.ContainsAny(input, "\t\r\n") {
		return Requirement{}, invalidRequirement(input)
	}
	if version, err := Parse(input); err == nil {
		return exactRequirement(input, version), nil
	}
	if strings.HasPrefix(input, "^") {
		version, err := parseRangeVersion(input[1:])
		if err != nil {
			return Requirement{}, invalidRequirement(input)
		}
		upper, ok := caretUpper(version)
		if !ok {
			return Requirement{}, invalidRequirement(input)
		}
		return rangeRequirement(input, Caret, version, true, upper, false), nil
	}
	if strings.HasPrefix(input, "~") {
		operand := input[1:]
		var version Version
		var err error
		if strings.Count(operand, ".") == 1 && !strings.ContainsAny(operand, "-+") {
			parts := strings.Split(operand, ".")
			if len(parts) != 2 || !canonicalNumber(parts[0]) || !canonicalNumber(parts[1]) {
				return Requirement{}, invalidRequirement(input)
			}
			version, err = Parse(operand + ".0")
		} else {
			version, err = parseRangeVersion(operand)
		}
		if err != nil || version.Minor == math.MaxUint64 {
			return Requirement{}, invalidRequirement(input)
		}
		upper, _ := Parse(strconv.FormatUint(version.Major, 10) + "." + strconv.FormatUint(version.Minor+1, 10) + ".0")
		return rangeRequirement(input, Tilde, version, true, upper, false), nil
	}
	if strings.Count(input, " ") == 1 {
		parts := strings.Split(input, " ")
		lowerOperator, lowerText := splitComparator(parts[0], true)
		upperOperator, upperText := splitComparator(parts[1], false)
		if lowerOperator == "" || upperOperator == "" {
			return Requirement{}, invalidRequirement(input)
		}
		lower, lowerErr := parseRangeVersion(lowerText)
		upper, upperErr := parseRangeVersion(upperText)
		if lowerErr != nil || upperErr != nil || Compare(lower, upper) >= 0 {
			return Requirement{}, invalidRequirement(input)
		}
		return rangeRequirement(input, ComparatorConjunction, lower, lowerOperator == ">=", upper, upperOperator == "<="), nil
	}
	return Requirement{}, invalidRequirement(input)
}

func canonicalNumber(value string) bool {
	if !allDigits(value) || (len(value) > 1 && value[0] == '0') {
		return false
	}
	_, err := strconv.ParseUint(value, 10, 64)
	return err == nil
}

func parseRangeVersion(value string) (Version, error) {
	version, err := Parse(value)
	if err != nil || len(version.Build) != 0 {
		return Version{}, invalidRequirement(value)
	}
	return version, nil
}

func splitComparator(value string, lower bool) (string, string) {
	operators := []string{">=", ">"}
	if !lower {
		operators = []string{"<=", "<"}
	}
	for _, operator := range operators {
		if strings.HasPrefix(value, operator) {
			return operator, value[len(operator):]
		}
	}
	return "", ""
}

func caretUpper(version Version) (Version, bool) {
	major, minor, patch := version.Major, version.Minor, version.Patch
	switch {
	case major != 0:
		if major == math.MaxUint64 {
			return Version{}, false
		}
		major++
		minor = 0
		patch = 0
	case minor != 0:
		if minor == math.MaxUint64 {
			return Version{}, false
		}
		minor++
		patch = 0
	default:
		if patch == math.MaxUint64 {
			return Version{}, false
		}
		patch++
	}
	upper, err := Parse(strconv.FormatUint(major, 10) + "." + strconv.FormatUint(minor, 10) + "." + strconv.FormatUint(patch, 10))
	return upper, err == nil
}

func exactRequirement(raw string, version Version) Requirement {
	copy := version
	requirement := Requirement{Raw: raw, Kind: Exact, Exact: &copy, prereleaseCores: map[string]struct{}{}}
	if version.IsPrerelease() {
		requirement.prereleaseCores[version.Core()] = struct{}{}
	}
	return requirement
}

func rangeRequirement(raw string, kind Kind, lower Version, lowerInclusive bool, upper Version, upperInclusive bool) Requirement {
	requirement := Requirement{
		Raw: raw, Kind: kind,
		Lower:           &Bound{Version: lower, Inclusive: lowerInclusive},
		Upper:           &Bound{Version: upper, Inclusive: upperInclusive},
		prereleaseCores: map[string]struct{}{},
	}
	if lower.IsPrerelease() {
		requirement.prereleaseCores[lower.Core()] = struct{}{}
	}
	if upper.IsPrerelease() {
		requirement.prereleaseCores[upper.Core()] = struct{}{}
	}
	return requirement
}

func invalidRequirement(input string) error {
	return fmt.Errorf("invalid_version_requirement: %q is not a canonical Seen v1 requirement", input)
}

func (requirement Requirement) Matches(version Version) bool {
	if requirement.Kind == Exact {
		return requirement.Exact != nil && requirement.Exact.String() == version.String()
	}
	if version.IsPrerelease() {
		if _, allowed := requirement.prereleaseCores[version.Core()]; !allowed {
			return false
		}
	}
	if requirement.Lower == nil || requirement.Upper == nil {
		return false
	}
	lower := Compare(version, requirement.Lower.Version)
	if lower < 0 || (lower == 0 && !requirement.Lower.Inclusive) {
		return false
	}
	upper := Compare(version, requirement.Upper.Version)
	if upper > 0 || (upper == 0 && !requirement.Upper.Inclusive) {
		return false
	}
	return true
}
