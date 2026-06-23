package main

import (
	_ "embed"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"slices"
	"strconv"
	"strings"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/app"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/dialog"
	"fyne.io/fyne/v2/layout"
	"fyne.io/fyne/v2/theme"
	"fyne.io/fyne/v2/widget"
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

func runHelper(proxyconfig *ProxyConfig, file_path string, enable bool, needAuth bool) {
	if needAuth {
		userLine := fmt.Sprintf("-user=%s", proxyconfig.user)
		passLine := fmt.Sprintf("-pass=%s", proxyconfig.pass)
		urlLine := fmt.Sprintf("-url=%s", proxyconfig.url)
		portLine := fmt.Sprintf("-port=%d", proxyconfig.port)
		noProxyLine := fmt.Sprintf("-noproxy=%s", proxyconfig.No_proxy)
		fileLine := fmt.Sprintf("-file=%s", file_path)
		enableStr := "true"
		if !enable {
			enableStr = "false"
		}
		enableLine := fmt.Sprintf("-enable=%s", enableStr)

		selfPath, _ := filepath.Abs(os.Args[0])
		cmd := exec.Command("pkexec", selfPath, "-apply",
			userLine, passLine, urlLine, portLine, noProxyLine, fileLine, enableLine)
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
		if err := cmd.Run(); err != nil {
			panic(err)
		}
		return
	}
	change_proxy_config(proxyconfig, file_path, enable)
}

//go:embed picture.png
var appIcon []byte

func main() {
	if len(os.Args) > 1 && os.Args[1] == "-apply" {
		var pc ProxyConfig
		var filePath string
		enable := true
		for _, arg := range os.Args[2:] {
			parts := strings.SplitN(arg, "=", 2)
			if len(parts) != 2 {
				continue
			}
			switch parts[0] {
			case "-user":
				pc.user = parts[1]
			case "-pass":
				pc.pass = parts[1]
			case "-url":
				pc.url = parts[1]
			case "-port":
				pc.port, _ = strconv.Atoi(parts[1])
			case "-noproxy":
				pc.No_proxy = parts[1]
			case "-file":
				filePath = parts[1]
			case "-enable":
				enable = parts[1] == "true"
			}
		}
		change_proxy_config(&pc, filePath, enable)
		return
	}

	err := os.Setenv("DESKTOP_APP_ID", "proxy-manager")
	if err != nil {
		println(err.Error())
	}
	a := app.NewWithID("proxy-manager")
	iconRes := fyne.NewStaticResource("logo", appIcon)
	a.SetIcon(iconRes)
	w := a.NewWindow("Configurador de Proxy")
	w.SetIcon(iconRes)

	userEntry := widget.NewEntry()
	userEntry.SetPlaceHolder("ej: usuario")

	passEntry := widget.NewPasswordEntry()
	passEntry.SetPlaceHolder("ej: contraseña")

	urlEntry := widget.NewEntry()
	urlEntry.SetPlaceHolder("ej: proxy.miempresa.com")

	portEntry := widget.NewEntry()
	portEntry.SetPlaceHolder("ej: 8080")

	noProxyEntry := widget.NewEntry()
	noProxyEntry.SetPlaceHolder("ej: localhost,127.0.0.1,.local")

	fileRadio := widget.NewRadioGroup([]string{"Bashrc", "/etc/environment", "Ambos"}, func(s string) {})
	fileRadio.SetSelected("Bashrc")

	enableRadio := widget.NewRadioGroup([]string{"Habilitar", "Deshabilitar"}, func(s string) {})
	enableRadio.SetSelected("Habilitar")

	applyBtn := widget.NewButtonWithIcon("Aplicar", theme.ConfirmIcon(), func() {
		port, err := strconv.Atoi(portEntry.Text)
		if err != nil {
			dialog.ShowError(fmt.Errorf("puerto inválido: debe ser un número"), w)
			return
		}

		pc := &ProxyConfig{
			user:     userEntry.Text,
			pass:     passEntry.Text,
			url:      urlEntry.Text,
			port:     port,
			No_proxy: noProxyEntry.Text,
		}

		enable := enableRadio.Selected == "Habilitar"

		var targets []string
		switch fileRadio.Selected {
		case "Ambos":
			targets = []string{Bashrc, EtcEnv}
		case "Bashrc":
			targets = []string{Bashrc}
		default:
			targets = []string{EtcEnv}
		}

		needsAuth := slices.Contains(targets, EtcEnv)

		if needsAuth {
			for _, f := range targets {
				runHelper(pc, f, enable, f == EtcEnv)
			}
		} else {
			for _, f := range targets {
				change_proxy_config(pc, f, enable)
			}
		}

		accion := "habilitado"
		if !enable {
			accion = "deshabilitado"
		}
		dialog.ShowInformation("Éxito", fmt.Sprintf("Proxy %s correctamente en %s", accion, fileRadio.Selected), w)
	})

	form := &widget.Form{
		Items: []*widget.FormItem{
			{Text: "Usuario", Widget: userEntry},
			{Text: "Contraseña", Widget: passEntry},
			{Text: "URL del Proxy", Widget: urlEntry},
			{Text: "Puerto", Widget: portEntry},
			{Text: "No Proxy", Widget: noProxyEntry},
		},
		SubmitText: "",
	}

	content := container.NewVBox(
		widget.NewLabelWithStyle("Configuración de Proxy", fyne.TextAlignCenter, fyne.TextStyle{Bold: true}),
		form,
		widget.NewSeparator(),
		widget.NewLabel("Archivo destino:"),
		fileRadio,
		widget.NewLabel("Acción:"),
		enableRadio,
		container.NewHBox(layout.NewSpacer(), applyBtn, layout.NewSpacer()),
	)

	w.SetContent(content)
	w.Resize(fyne.NewSize(480, 520))
	w.ShowAndRun()
}
