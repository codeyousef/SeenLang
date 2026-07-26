package commands

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/codeyousef/seen/tools/seen-pkg/internal/atomicfile"
	"github.com/codeyousef/seen/tools/seen-pkg/internal/model"
)

type PackageMapRow struct{ RequesterRoot, Alias, DependencyRoot string }

// WritePackageMap promotes a complete resolution map only after every row has
// been checked. Callers must invoke it after graph, metadata, archive, and lock
// verification; no partial map is observable.
func WritePackageMap(projectRoot string, rows []PackageMapRow) error {
	if !filepath.IsAbs(projectRoot) || filepath.Clean(projectRoot) != projectRoot {
		return fmt.Errorf("project root must be a canonical absolute path")
	}
	resolvedProject, err := filepath.EvalSymlinks(projectRoot)
	if err != nil || resolvedProject != projectRoot {
		return fmt.Errorf("project root must be a real canonical path")
	}
	stateDir := filepath.Join(projectRoot, ".seen")
	if err := ensureRealDirectory(stateDir, 0o700); err != nil {
		return err
	}
	viewsRoot := filepath.Join(stateDir, "views")
	if err := ensureRealDirectory(viewsRoot, 0o700); err != nil {
		return err
	}
	canonicalViews, err := filepath.EvalSymlinks(viewsRoot)
	if err != nil {
		return fmt.Errorf("resolve package views: %w", err)
	}
	canonicalViews, err = filepath.Abs(canonicalViews)
	if err != nil {
		return err
	}
	canonical := append([]PackageMapRow(nil), rows...)
	seen := map[PackageMapRow]struct{}{}
	for index, row := range canonical {
		if err := model.ValidateAlias(row.Alias); err != nil {
			return fmt.Errorf("row %d alias: %w", index, err)
		}
		if row.RequesterRoot != projectRoot {
			if err := validateViewRoot(canonicalViews, row.RequesterRoot); err != nil {
				return fmt.Errorf("row %d requester_root: %w", index, err)
			}
		} else if strings.ContainsAny(row.RequesterRoot, "\t\r\n") {
			return fmt.Errorf("row %d requester_root contains a TSV delimiter", index)
		}
		if err := validateViewRoot(canonicalViews, row.DependencyRoot); err != nil {
			return fmt.Errorf("row %d dependency_root: %w", index, err)
		}
		if strings.ContainsAny(row.RequesterRoot+row.Alias+row.DependencyRoot, "\t\r\n") {
			return fmt.Errorf("row %d contains a TSV delimiter", index)
		}
		if _, duplicate := seen[row]; duplicate {
			return fmt.Errorf("duplicate package-map row %d", index)
		}
		seen[row] = struct{}{}
	}
	sort.Slice(canonical, func(i, j int) bool {
		if canonical[i].RequesterRoot != canonical[j].RequesterRoot {
			return canonical[i].RequesterRoot < canonical[j].RequesterRoot
		}
		if canonical[i].Alias != canonical[j].Alias {
			return canonical[i].Alias < canonical[j].Alias
		}
		return canonical[i].DependencyRoot < canonical[j].DependencyRoot
	})
	var output strings.Builder
	for _, row := range canonical {
		output.WriteString(row.RequesterRoot)
		output.WriteByte('\t')
		output.WriteString(row.Alias)
		output.WriteByte('\t')
		output.WriteString(row.DependencyRoot)
		output.WriteByte('\n')
	}
	temp, err := os.CreateTemp(stateDir, ".package-map.tmp-*")
	if err != nil {
		return err
	}
	tempName := temp.Name()
	ok := false
	defer func() {
		if !ok {
			_ = os.Remove(tempName)
		}
	}()
	if err := temp.Chmod(0o600); err != nil {
		_ = temp.Close()
		return err
	}
	if _, err := temp.WriteString(output.String()); err != nil {
		_ = temp.Close()
		return err
	}
	if err := temp.Sync(); err != nil {
		_ = temp.Close()
		return err
	}
	if err := temp.Close(); err != nil {
		return err
	}
	if err := atomicfile.Replace(tempName, filepath.Join(stateDir, "package-map.tsv")); err != nil {
		return fmt.Errorf("promote package map: %w", err)
	}
	if err := atomicfile.SyncDir(stateDir); err != nil {
		return err
	}
	ok = true
	return nil
}

// RemovePackageMap removes a previously authoritative graph when the manifest
// contains no package import edges. It never follows a project-state symlink.
func RemovePackageMap(projectRoot string) error {
	if !filepath.IsAbs(projectRoot) || filepath.Clean(projectRoot) != projectRoot {
		return fmt.Errorf("project root must be a canonical absolute path")
	}
	resolvedProject, err := filepath.EvalSymlinks(projectRoot)
	if err != nil || resolvedProject != projectRoot {
		return fmt.Errorf("project root must be a real canonical path")
	}
	stateDir := filepath.Join(projectRoot, ".seen")
	info, err := os.Lstat(stateDir)
	if errors.Is(err, os.ErrNotExist) {
		return nil
	}
	if err != nil || !info.IsDir() || info.Mode()&os.ModeSymlink != 0 {
		return fmt.Errorf("package state path %s must be a real directory", stateDir)
	}
	return removeMutableFile(filepath.Join(stateDir, "package-map.tsv"))
}

func removeMutableFile(filename string) error {
	err := os.Remove(filename)
	if errors.Is(err, os.ErrNotExist) {
		return nil
	}
	if err != nil {
		return err
	}
	return atomicfile.SyncDir(filepath.Dir(filename))
}

func ensureRealDirectory(name string, mode os.FileMode) error {
	info, err := os.Lstat(name)
	if errors.Is(err, os.ErrNotExist) {
		if err := os.Mkdir(name, mode); err != nil {
			return fmt.Errorf("create package state directory: %w", err)
		}
		info, err = os.Lstat(name)
	}
	if err != nil || !info.IsDir() || info.Mode()&os.ModeSymlink != 0 {
		return fmt.Errorf("package state path %s must be a real directory", name)
	}
	return nil
}

func validateViewRoot(viewsRoot, value string) error {
	if !filepath.IsAbs(value) || filepath.Clean(value) != value {
		return fmt.Errorf("must be a canonical absolute path")
	}
	canonical, err := filepath.EvalSymlinks(value)
	if err != nil {
		return fmt.Errorf("resolve view root: %w", err)
	}
	canonical, err = filepath.Abs(canonical)
	if err != nil {
		return err
	}
	if canonical != value {
		return fmt.Errorf("must not traverse a symbolic link")
	}
	relative, err := filepath.Rel(viewsRoot, canonical)
	if err != nil {
		return err
	}
	if relative == "." || relative == ".." || strings.HasPrefix(relative, ".."+string(filepath.Separator)) {
		return fmt.Errorf("must be below %s", viewsRoot)
	}
	info, err := os.Stat(canonical)
	if err != nil {
		return err
	}
	if !info.IsDir() {
		return fmt.Errorf("must be a directory")
	}
	if info.Mode().Perm()&0o222 != 0 {
		return fmt.Errorf("view root must be read-only")
	}
	return nil
}
