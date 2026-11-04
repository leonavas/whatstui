package main

import (
	"os"
	"strings"
)

// The interface's language, as whatstui passes it in WHATSTUI_LANG: English
// unless it says pt-BR. Every text the user sees goes through tr.
var langPT = func() bool {
	v := strings.ToLower(os.Getenv("WHATSTUI_LANG"))
	return v == "pt-br" || v == "pt" || v == "pt_br"
}()

// tr returns the English or the Brazilian Portuguese text, for the language in use.
func tr(en, pt string) string {
	if langPT {
		return pt
	}
	return en
}
