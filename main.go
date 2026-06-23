package main

import (
	"fmt"
	"os"
	"strings"
)

type ProxyConfig struct {
	user     string
	pass     string
	url      string
	port     int
	No_proxy string
}

const (
	EtcEnv = "/etc/environment"
	Bashrc = "/home/julio/.bashrc"
)

func (pc *ProxyConfig) FullUrlBuilder() string {
	return fmt.Sprintf("\"http://%s:%s@%s:%d\"", pc.user, pc.pass, pc.url, pc.port)
}

func (pc *ProxyConfig) NoProxyBuilder() string {
	return fmt.Sprintf("\"%s\"", pc.No_proxy)
}

func change_proxy_config(proxyconfig *ProxyConfig, file_path string, enable bool) {
	data, err := os.ReadFile(file_path)
	if err != nil {
		panic(err)
	}
	lineas := strings.Split(string(data), "\n")

	http_proxy := "http_proxy="
	https_proxy := "https_proxy="
	no_proxy := "no_proxy="

	if file_path == Bashrc {
		http_proxy = fmt.Sprintf("%s%s", "export ", http_proxy)
		https_proxy = fmt.Sprintf("%s%s", "export ", https_proxy)
		no_proxy = fmt.Sprintf("%s%s", "export ", no_proxy)
	}

	full_url := proxyconfig.FullUrlBuilder()
	noproxy := proxyconfig.NoProxyBuilder()

	comment := "# "

	if enable {
		comment = ""
	}

	for i, li := range lineas {
		if strings.Contains(li, http_proxy) {
			lineas[i] = fmt.Sprintf("%s%s%s", comment, http_proxy, full_url)
			continue
		}
		if strings.Contains(li, https_proxy) {
			lineas[i] = fmt.Sprintf("%s%s%s", comment, https_proxy, full_url)
			continue
		}
		if strings.Contains(li, no_proxy) {
			lineas[i] = fmt.Sprintf("%s%s%s", comment, no_proxy, noproxy)
			continue
		}
		if strings.Contains(li, "proxy") {
			lineas[i] = ""
		}

		err := os.WriteFile(file_path, []byte(strings.Join(lineas, "\n")), 0644)

		if err != nil {
			panic(err)
		}
	}
}

func main() {
	// proxy := ProxyConfig{user: "julioa", pass: "12", url: "12.18.1.9", port: 38}
	//
	// change_proxy_config(&proxy, Bashrc, false)
	// change_proxy_config(&proxy, EtcEnv, false)
}
