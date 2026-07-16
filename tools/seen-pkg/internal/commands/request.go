package commands

import (
	"bufio"
	"fmt"
	"io"
	"os"
	"strconv"
	"strings"
	"unicode/utf8"
)

const (
	ProtocolVersion  = "SEENPKG1"
	SidecarVersion   = "0.10.0"
	maxRequestBytes  = 8 << 20
	maxRequestArgs   = 4096
	maxArgumentBytes = 1 << 20
)

func ReadRequest(filename string) ([]string, error) {
	file, err := os.Open(filename)
	if err != nil {
		return nil, fmt.Errorf("open sidecar request: %w", err)
	}
	defer file.Close()
	info, err := file.Stat()
	if err != nil {
		return nil, fmt.Errorf("stat sidecar request: %w", err)
	}
	if !info.Mode().IsRegular() {
		return nil, fmt.Errorf("sidecar request must be a regular file")
	}
	if info.Size() > maxRequestBytes {
		return nil, fmt.Errorf("sidecar request exceeds %d bytes", maxRequestBytes)
	}
	return DecodeRequest(io.LimitReader(file, maxRequestBytes+1))
}

func DecodeRequest(input io.Reader) ([]string, error) {
	reader := bufio.NewReaderSize(input, 32*1024)
	protocol, err := readLine(reader)
	if err != nil {
		return nil, fmt.Errorf("read request protocol: %w", err)
	}
	if protocol != ProtocolVersion {
		return nil, fmt.Errorf("unsupported sidecar protocol %q", protocol)
	}
	countLine, err := readLine(reader)
	if err != nil {
		return nil, fmt.Errorf("read request argument count: %w", err)
	}
	count, err := parseCanonicalDecimal(countLine, maxRequestArgs)
	if err != nil {
		return nil, fmt.Errorf("invalid request argument count: %w", err)
	}
	arguments := make([]string, 0, count)
	for index := 0; index < count; index++ {
		lengthLine, err := readLine(reader)
		if err != nil {
			return nil, fmt.Errorf("read argument %d length: %w", index, err)
		}
		length, err := parseCanonicalDecimal(lengthLine, maxArgumentBytes)
		if err != nil {
			return nil, fmt.Errorf("invalid argument %d length: %w", index, err)
		}
		content := make([]byte, length)
		if _, err := io.ReadFull(reader, content); err != nil {
			return nil, fmt.Errorf("argument %d is truncated: %w", index, err)
		}
		delimiter, err := reader.ReadByte()
		if err != nil || delimiter != '\n' {
			return nil, fmt.Errorf("argument %d is missing its newline delimiter", index)
		}
		if !utf8.Valid(content) {
			return nil, fmt.Errorf("argument %d is not valid UTF-8", index)
		}
		if strings.IndexByte(string(content), 0) >= 0 {
			return nil, fmt.Errorf("argument %d contains NUL", index)
		}
		arguments = append(arguments, string(content))
	}
	if trailing, err := reader.ReadByte(); err != io.EOF {
		if err == nil {
			return nil, fmt.Errorf("request has trailing data beginning with byte %d", trailing)
		}
		return nil, fmt.Errorf("read request trailer: %w", err)
	}
	return arguments, nil
}

func readLine(reader *bufio.Reader) (string, error) {
	line, err := reader.ReadString('\n')
	if err != nil {
		return "", err
	}
	if len(line) > 64 {
		return "", fmt.Errorf("header line is too long")
	}
	line = strings.TrimSuffix(line, "\n")
	if strings.ContainsRune(line, '\r') {
		return "", fmt.Errorf("CR is not canonical")
	}
	return line, nil
}

func parseCanonicalDecimal(value string, maximum int) (int, error) {
	if value == "" || (len(value) > 1 && value[0] == '0') {
		return 0, fmt.Errorf("non-canonical decimal %q", value)
	}
	for _, c := range value {
		if c < '0' || c > '9' {
			return 0, fmt.Errorf("non-canonical decimal %q", value)
		}
	}
	number, err := strconv.ParseUint(value, 10, 31)
	if err != nil || number > uint64(maximum) {
		return 0, fmt.Errorf("value exceeds %d", maximum)
	}
	return int(number), nil
}
