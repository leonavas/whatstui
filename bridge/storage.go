package main

import (
	"fmt"
	"os"
	"path/filepath"
	"syscall"
)

// Also protect direct helper startup and data left by older versions.
func privateDataDir(dir string) error {
	if err := os.MkdirAll(dir, 0o700); err != nil {
		return err
	}
	d, err := os.OpenFile(dir, os.O_RDONLY|syscall.O_DIRECTORY|syscall.O_NOFOLLOW, 0)
	if err != nil {
		return err
	}
	err = d.Chmod(0o700)
	d.Close()
	if err != nil {
		return err
	}
	entries, err := os.ReadDir(dir)
	if err != nil {
		return err
	}
	for _, entry := range entries {
		if entry.IsDir() {
			continue
		}
		f, err := os.OpenFile(filepath.Join(dir, entry.Name()), os.O_RDONLY|syscall.O_NOFOLLOW|syscall.O_NONBLOCK, 0)
		if os.IsNotExist(err) {
			continue // A running SQLite connection may have removed a journal.
		}
		if err != nil {
			return err
		}
		info, err := f.Stat()
		if err == nil && (!info.Mode().IsRegular() || info.Sys().(*syscall.Stat_t).Nlink != 1) {
			err = fmt.Errorf("account data must be a regular file without hard links: %s", entry.Name())
		}
		if err == nil {
			err = f.Chmod(0o600)
		}
		f.Close()
		if err != nil {
			return err
		}
	}
	return nil
}
