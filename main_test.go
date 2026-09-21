package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestUppercaseProxyVars(t *testing.T) {
	dir := t.TempDir()
	bashrc := filepath.Join(dir, ".bashrc")
	etcEnv := filepath.Join(dir, "environment")
	missing := filepath.Join(dir, "missing") // no existe, para que ensure la salte y no use pkexec

	os.WriteFile(bashrc, []byte(`# mi bashrc
export HTTP_PROXY="http://old:p@oldproxy:3128"
# export HTTPS_PROXY=""
`), 0644)

	os.WriteFile(etcEnv, []byte(`http_proxy="http://old:p@oldproxy:3128"
# https_proxy=""
# HTTPS_PROXY=""
# no_proxy=""
NO_PROXY="localhost"
# HTTP_PROXY=""
`), 0644)

	origFiles := proxyTargetFiles
	origTerm := TermConfigFile
	defer func() {
		proxyTargetFiles = origFiles
		TermConfigFile = origTerm
	}()

	// 1) ensureProxyVarsExist solo sobre bashrc (el segundo path no existe)
	TermConfigFile = bashrc
	proxyTargetFiles = []string{bashrc, missing}
	ensureProxyVarsExist()

	data, _ := os.ReadFile(bashrc)
	out := string(data)
	t.Logf("bashrc tras ensure:\n%s", out)
	for _, want := range []string{
		"# export http_proxy=\"\"",
		"# export https_proxy=\"\"",
		"# export no_proxy=\"\"",
		"# export NO_PROXY=\"\"",
	} {
		if !strings.Contains(out, want) {
			t.Errorf("falta stub %q tras ensure", want)
		}
	}

	// 2) parse desde mayúsculas
	pcBash, okBash := parseConfigFromFile(bashrc)
	if !okBash {
		t.Fatalf("no parseó el bashrc con HTTP_PROXY en mayúsculas")
	}
	if pcBash.url != "oldproxy" || pcBash.port != 3128 {
		t.Errorf("parse incorrecto: %+v", pcBash)
	}

	pcEtc, okEtc := parseConfigFromFile(etcEnv)
	if !okEtc {
		t.Fatalf("no parseó etcEnv")
	}
	if pcEtc.No_proxy != "localhost" {
		t.Errorf("NO_PROXY no parseada: %+v", pcEtc)
	}

	// 3) habilitar en ambos archivos
	proxyTargetFiles = []string{bashrc, etcEnv}
	pc := &ProxyConfig{user: "u", pass: "p", url: "proxy.test.com", port: 8080, No_proxy: "localhost,127.0.0.1"}
	change_proxy_config(pc, bashrc, true)
	change_proxy_config(pc, etcEnv, true)

	dataB, _ := os.ReadFile(bashrc)
	t.Logf("bashrc habilitado:\n%s", dataB)
	dataE, _ := os.ReadFile(etcEnv)
	t.Logf("etcEnv habilitado:\n%s", dataE)

	if !strings.Contains(string(dataB), `export HTTP_PROXY="http://u:p@proxy.test.com:8080"`) {
		t.Errorf("HTTP_PROXY no actualizada en bashrc")
	}
	if !strings.Contains(string(dataB), `export HTTPS_PROXY="http://u:p@proxy.test.com:8080"`) {
		t.Errorf("HTTPS_PROXY no activada en bashrc")
	}
	if !strings.Contains(string(dataE), `NO_PROXY="localhost,127.0.0.1"`) {
		t.Errorf("NO_PROXY no actualizada en etcEnv")
	}
	if !strings.Contains(string(dataE), `HTTP_PROXY="http://u:p@proxy.test.com:8080"`) {
		t.Errorf("HTTP_PROXY no activada en etcEnv")
	}

	if !isProxyEnabled() {
		t.Errorf("isProxyEnabled debería detectar el proxy habilitado (mayúsculas)")
	}

	// 4) deshabilitar
	change_proxy_config(pc, bashrc, false)
	change_proxy_config(pc, etcEnv, false)

	dataB2, _ := os.ReadFile(bashrc)
	t.Logf("bashrc deshabilitado:\n%s", dataB2)
	dataE2, _ := os.ReadFile(etcEnv)
	t.Logf("etcEnv deshabilitado:\n%s", dataE2)

	if strings.Contains(string(dataB2), "\nexport HTTP_PROXY=") {
		t.Errorf("HTTP_PROXY debería quedar comentada tras deshabilitar")
	}
	if !strings.Contains(string(dataB2), `# export HTTP_PROXY="http://u:p@proxy.test.com:8080"`) {
		t.Errorf("falta la línea comentada de HTTP_PROXY")
	}
	if strings.Contains(string(dataE2), "\nNO_PROXY=") {
		t.Errorf("NO_PROXY debería quedar comentada tras deshabilitar")
	}

	if isProxyEnabled() {
		t.Errorf("isProxyEnabled debería dar false tras deshabilitar")
	}
}
