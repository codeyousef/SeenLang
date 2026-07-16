package main

import (
	"context"
	"fmt"
	"os"

	"github.com/codeyousef/seen/tools/seen-pkg/internal/commands"
)

func main() {
	arguments := os.Args[1:]
	if len(arguments) > 0 && arguments[0] == "--request" {
		if len(arguments) != 2 {
			fmt.Fprintln(os.Stderr, "seen-pkg: --request requires exactly one path")
			os.Exit(64)
		}
		decoded, err := commands.ReadRequest(arguments[1])
		if err != nil {
			fmt.Fprintln(os.Stderr, "seen-pkg: invalid request:", err)
			os.Exit(65)
		}
		arguments = decoded
	}
	runner := commands.Runner{Backend: commands.NewProductionBackend(), Streams: commands.Streams{Stdout: os.Stdout, Stderr: os.Stderr}}
	os.Exit(runner.Run(context.Background(), arguments))
}
