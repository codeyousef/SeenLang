package tuf

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"sort"
	"strconv"
	"strings"
	"unicode/utf8"
)

// CanonicalJSON implements seen-tuf-canonical-json-v1: UTF-8 JSON, duplicate
// member rejection, byte-sorted object keys, no insignificant whitespace, and
// canonical integer spelling. Metadata intentionally has no floating values.
func CanonicalJSON(raw []byte) ([]byte, error) {
	if !utf8.Valid(raw) {
		return nil, errors.New("metadata is not valid UTF-8")
	}
	if err := validateSurrogateEscapes(raw); err != nil {
		return nil, err
	}
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.UseNumber()
	value, err := parseJSONValue(decoder)
	if err != nil {
		return nil, err
	}
	if token, err := decoder.Token(); !errors.Is(err, io.EOF) {
		if err == nil {
			return nil, fmt.Errorf("trailing JSON token %v", token)
		}
		return nil, err
	}
	var out bytes.Buffer
	if err := writeCanonical(&out, value); err != nil {
		return nil, err
	}
	return out.Bytes(), nil
}

func validateSurrogateEscapes(raw []byte) error {
	inString := false
	for i := 0; i < len(raw); i++ {
		switch raw[i] {
		case '"':
			inString = !inString
		case '\\':
			if !inString || i+1 >= len(raw) {
				continue
			}
			if raw[i+1] != 'u' {
				i++
				continue
			}
			if i+6 > len(raw) {
				return errors.New("truncated Unicode escape")
			}
			value, err := strconv.ParseUint(string(raw[i+2:i+6]), 16, 16)
			if err != nil {
				return errors.New("invalid Unicode escape")
			}
			if value >= 0xdc00 && value <= 0xdfff {
				return errors.New("unpaired low surrogate")
			}
			if value >= 0xd800 && value <= 0xdbff {
				if i+12 > len(raw) || raw[i+6] != '\\' || raw[i+7] != 'u' {
					return errors.New("unpaired high surrogate")
				}
				low, err := strconv.ParseUint(string(raw[i+8:i+12]), 16, 16)
				if err != nil || low < 0xdc00 || low > 0xdfff {
					return errors.New("invalid surrogate pair")
				}
				i += 11
				continue
			}
			i += 5
		}
	}
	return nil
}

func parseJSONValue(decoder *json.Decoder) (any, error) {
	token, err := decoder.Token()
	if err != nil {
		return nil, err
	}
	delim, isDelim := token.(json.Delim)
	if !isDelim {
		switch value := token.(type) {
		case nil, bool, string, json.Number:
			return value, nil
		default:
			return nil, fmt.Errorf("unsupported JSON token %T", token)
		}
	}
	switch delim {
	case '{':
		object := make(map[string]any)
		for decoder.More() {
			keyToken, err := decoder.Token()
			if err != nil {
				return nil, err
			}
			key, ok := keyToken.(string)
			if !ok {
				return nil, errors.New("object key is not a string")
			}
			if _, duplicate := object[key]; duplicate {
				return nil, fmt.Errorf("duplicate JSON member %q", key)
			}
			value, err := parseJSONValue(decoder)
			if err != nil {
				return nil, err
			}
			object[key] = value
		}
		end, err := decoder.Token()
		if err != nil || end != json.Delim('}') {
			return nil, errors.New("unterminated JSON object")
		}
		return object, nil
	case '[':
		var array []any
		for decoder.More() {
			value, err := parseJSONValue(decoder)
			if err != nil {
				return nil, err
			}
			array = append(array, value)
		}
		end, err := decoder.Token()
		if err != nil || end != json.Delim(']') {
			return nil, errors.New("unterminated JSON array")
		}
		return array, nil
	default:
		return nil, fmt.Errorf("unexpected JSON delimiter %q", delim)
	}
}

func writeCanonical(out *bytes.Buffer, value any) error {
	switch value := value.(type) {
	case nil:
		out.WriteString("null")
	case bool:
		if value {
			out.WriteString("true")
		} else {
			out.WriteString("false")
		}
	case string:
		writeJSONString(out, value)
	case json.Number:
		number := value.String()
		if strings.ContainsAny(number, ".eE+") {
			return fmt.Errorf("non-integer JSON number %q", number)
		}
		parsed, err := strconv.ParseInt(number, 10, 64)
		if err != nil || strconv.FormatInt(parsed, 10) != number {
			return fmt.Errorf("non-canonical JSON integer %q", number)
		}
		out.WriteString(number)
	case []any:
		out.WriteByte('[')
		for i, item := range value {
			if i != 0 {
				out.WriteByte(',')
			}
			if err := writeCanonical(out, item); err != nil {
				return err
			}
		}
		out.WriteByte(']')
	case map[string]any:
		keys := make([]string, 0, len(value))
		for key := range value {
			keys = append(keys, key)
		}
		sort.Strings(keys)
		out.WriteByte('{')
		for i, key := range keys {
			if i != 0 {
				out.WriteByte(',')
			}
			writeJSONString(out, key)
			out.WriteByte(':')
			if err := writeCanonical(out, value[key]); err != nil {
				return err
			}
		}
		out.WriteByte('}')
	default:
		return fmt.Errorf("unsupported canonical JSON value %T", value)
	}
	return nil
}

func writeJSONString(out *bytes.Buffer, value string) {
	const hexDigits = "0123456789abcdef"
	out.WriteByte('"')
	for _, r := range value {
		switch r {
		case '"':
			out.WriteString(`\"`)
		case '\\':
			out.WriteString(`\\`)
		case '\b':
			out.WriteString(`\b`)
		case '\t':
			out.WriteString(`\t`)
		case '\n':
			out.WriteString(`\n`)
		case '\f':
			out.WriteString(`\f`)
		case '\r':
			out.WriteString(`\r`)
		default:
			if r >= 0 && r < 0x20 {
				out.WriteString(`\u00`)
				out.WriteByte(hexDigits[byte(r)>>4])
				out.WriteByte(hexDigits[byte(r)&0x0f])
			} else {
				out.WriteRune(r)
			}
		}
	}
	out.WriteByte('"')
}
