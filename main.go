package main

import (
	_ "embed"
	"encoding/json"
	"fmt"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"slices"
	"strconv"
	"strings"
	"time"

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
	EtcEnv    = "/etc/environment"
	Bashrc    = "/home/julio/.bashrc"
	HistFile  = "/home/julio/.proxy-manager-history.json"
)

type HistoryEntry struct {
	Timestamp string `json:"timestamp"`
	User      string `json:"user"`
	Pass      string `json:"pass"`
	Url       string `json:"url"`
	Port      int    `json:"port"`
	NoProxy   string `json:"no_proxy"`
	Action    string `json:"action"`
	Files     string `json:"files"`
}

type Settings struct {
	WindowWidth  float32 `json:"window_width"`
	WindowHeight float32 `json:"window_height"`
	SplitOffset  float64 `json:"split_offset"`
}

func settingsFilePath() string {
	if home := os.Getenv("HOME"); home != "" {
		return filepath.Join(home, ".proxy-manager-settings.json")
	}
	return "/home/julio/.proxy-manager-settings.json"
}

func loadSettings() Settings {
	path := settingsFilePath()
	data, err := os.ReadFile(path)
	if err != nil {
		return Settings{WindowWidth: 800, WindowHeight: 520, SplitOffset: 0.35}
	}
	var s Settings
	if err := json.Unmarshal(data, &s); err != nil {
		return Settings{WindowWidth: 800, WindowHeight: 520, SplitOffset: 0.35}
	}
	return s
}

func saveSettings(s Settings) error {
	path := settingsFilePath()
	data, err := json.MarshalIndent(s, "", "  ")
	if err != nil {
		return err
	}
	return os.WriteFile(path, data, 0644)
}

func historyFilePath() string {
	if home := os.Getenv("HOME"); home != "" {
		return filepath.Join(home, ".proxy-manager-history.json")
	}
	return HistFile
}

func loadHistory() []HistoryEntry {
	path := historyFilePath()
	data, err := os.ReadFile(path)
	if err != nil {
		return []HistoryEntry{}
	}
	var hist []HistoryEntry
	if err := json.Unmarshal(data, &hist); err != nil {
		return []HistoryEntry{}
	}
	return hist
}

func saveHistory(hist []HistoryEntry) error {
	path := historyFilePath()
	data, err := json.MarshalIndent(hist, "", "  ")
	if err != nil {
		return err
	}
	return os.WriteFile(path, data, 0644)
}

func addHistoryEntry(entry HistoryEntry) error {
	hist := loadHistory()
	hist = append(hist, entry)
	return saveHistory(hist)
}

func parseConfigFromFile(filePath string) (*ProxyConfig, bool) {
	data, err := os.ReadFile(filePath)
	if err != nil {
		return nil, false
	}
	lines := strings.Split(string(data), "\n")

	var proxyURLStr, noProxyStr string
	var found bool

	for _, line := range lines {
		trimmed := strings.TrimSpace(line)
		if trimmed == "" || strings.HasPrefix(trimmed, "#") {
			continue
		}

		// Quitar prefijo 'export ' si existe (para .bashrc)
		cleanLine := strings.TrimPrefix(trimmed, "export ")

		if strings.HasPrefix(cleanLine, "http_proxy=") || strings.HasPrefix(cleanLine, "https_proxy=") {
			parts := strings.SplitN(cleanLine, "=", 2)
			if len(parts) == 2 {
				proxyURLStr = strings.Trim(parts[1], `"`)
				found = true
			}
		} else if strings.HasPrefix(cleanLine, "no_proxy=") {
			parts := strings.SplitN(cleanLine, "=", 2)
			if len(parts) == 2 {
				noProxyStr = strings.Trim(parts[1], `"`)
			}
		}
	}

	if !found || proxyURLStr == "" {
		return nil, false
	}

	u, err := url.Parse(proxyURLStr)
	if err != nil {
		return nil, false
	}

	port, _ := strconv.Atoi(u.Port())
	pass, _ := u.User.Password()

	return &ProxyConfig{
		user:     u.User.Username(),
		pass:     pass,
		url:      u.Hostname(),
		port:     port,
		No_proxy: noProxyStr,
	}, true
}

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

	var historyData []HistoryEntry
	var historyList *widget.List

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

	// Detectar proxies activos al iniciar
	pcBash, activeBash := parseConfigFromFile(Bashrc)
	pcEtc, activeEtc := parseConfigFromFile(EtcEnv)

	if activeBash || activeEtc {
		var targetPC *ProxyConfig
		if activeBash {
			targetPC = pcBash
		} else {
			targetPC = pcEtc
		}

		userEntry.SetText(targetPC.user)
		passEntry.SetText(targetPC.pass)
		urlEntry.SetText(targetPC.url)
		portEntry.SetText(strconv.Itoa(targetPC.port))
		noProxyEntry.SetText(targetPC.No_proxy)
		enableRadio.SetSelected("Habilitar")

		if activeBash && activeEtc {
			fileRadio.SetSelected("Ambos")
		} else if activeBash {
			fileRadio.SetSelected("Bashrc")
		} else {
			fileRadio.SetSelected("/etc/environment")
		}
	} else {
		enableRadio.SetSelected("Deshabilitar")
		portEntry.SetText("3128")
	}

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

	// Botón para guardar en el almacén
	saveBtn := widget.NewButtonWithIcon("Guardar Config", theme.DocumentSaveIcon(), func() {
		port, err := strconv.Atoi(portEntry.Text)
		if err != nil {
			dialog.ShowError(fmt.Errorf("puerto inválido: debe ser un número"), w)
			return
		}

		actionStr := "Habilitar"
		if enableRadio.Selected == "Deshabilitar" {
			actionStr = "Deshabilitar"
		}

		entry := HistoryEntry{
			Timestamp: time.Now().Format(time.RFC3339),
			User:      userEntry.Text,
			Pass:      passEntry.Text,
			Url:       urlEntry.Text,
			Port:      port,
			NoProxy:   noProxyEntry.Text,
			Action:    actionStr,
			Files:     fileRadio.Selected,
		}
		if err := addHistoryEntry(entry); err != nil {
			dialog.ShowError(fmt.Errorf("error guardando configuración: %v", err), w)
			return
		}
		historyData = loadHistory()
		if historyList != nil {
			historyList.UnselectAll()
			historyList.Refresh()
		}
	})

	// Configuraciones Guardadas
	historyData = loadHistory()
	historyList = widget.NewList(
		func() int { return len(historyData) },
		func() fyne.CanvasObject {
			btn := widget.NewButtonWithIcon("", theme.DeleteIcon(), func() {})
			btn.Importance = widget.LowImportance
			return container.NewHBox(
				widget.NewLabel("template"),
				layout.NewSpacer(),
				btn,
			)
		},
		func(i widget.ListItemID, o fyne.CanvasObject) {
			row := o.(*fyne.Container)
			label := row.Objects[0].(*widget.Label)
			btn := row.Objects[2].(*widget.Button)

			entry := historyData[i]
			label.SetText(fmt.Sprintf("%s | %s", entry.User, entry.Url))

			id := i
			btn.OnTapped = func() {
				entries := loadHistory()
				if id < len(entries) {
					entries = append(entries[:id], entries[id+1:]...)
					if err := saveHistory(entries); err != nil {
						dialog.ShowError(fmt.Errorf("error eliminando: %v", err), w)
						return
					}
					historyData = entries
					historyList.UnselectAll()
					historyList.Refresh()
				}
			}
		},
	)
	historyList.OnSelected = func(id widget.ListItemID) {
		entry := historyData[id]
		userEntry.SetText(entry.User)
		passEntry.SetText(entry.Pass)
		urlEntry.SetText(entry.Url)
		portEntry.SetText(strconv.Itoa(entry.Port))
		noProxyEntry.SetText(entry.NoProxy)
		if entry.Action == "Habilitar" {
			enableRadio.SetSelected("Habilitar")
		} else {
			enableRadio.SetSelected("Deshabilitar")
		}
		if entry.Files == "Bashrc" {
			fileRadio.SetSelected("Bashrc")
		} else if entry.Files == "Ambos" {
			fileRadio.SetSelected("Ambos")
		} else {
			fileRadio.SetSelected("/etc/environment")
		}
	}

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

	historyContent := container.NewBorder(
		widget.NewLabelWithStyle("Configuraciones Guardadas", fyne.TextAlignCenter, fyne.TextStyle{Bold: true}),
		nil, nil, nil,
		historyList,
	)

	formContent := container.NewVBox(
		widget.NewLabelWithStyle("Configuración de Proxy", fyne.TextAlignCenter, fyne.TextStyle{Bold: true}),
		form,
		widget.NewSeparator(),
		widget.NewLabel("Archivo destino:"),
		fileRadio,
		widget.NewLabel("Acción:"),
		enableRadio,
		container.NewHBox(layout.NewSpacer(), saveBtn, applyBtn, layout.NewSpacer()),
	)

	split := container.NewHSplit(historyContent, formContent)

	settings := loadSettings()
	split.SetOffset(settings.SplitOffset)
	w.Resize(fyne.NewSize(settings.WindowWidth, settings.WindowHeight))

	w.SetContent(split)
	w.SetOnClosed(func() {
		saveSettings(Settings{
			WindowWidth:  w.Canvas().Size().Width,
			WindowHeight: w.Canvas().Size().Height,
			SplitOffset:  split.Offset,
		})
	})

	go func() {
		lastWidth := w.Canvas().Size().Width
		lastHeight := w.Canvas().Size().Height
		lastOffset := split.Offset
		for {
			time.Sleep(500 * time.Millisecond)
			currentWidth := w.Canvas().Size().Width
			currentHeight := w.Canvas().Size().Height
			currentOffset := split.Offset
			if currentWidth != lastWidth || currentHeight != lastHeight || currentOffset != lastOffset {
				saveSettings(Settings{
					WindowWidth:  currentWidth,
					WindowHeight: currentHeight,
					SplitOffset:  currentOffset,
				})
				lastWidth = currentWidth
				lastHeight = currentHeight
				lastOffset = currentOffset
			}
		}
	}()

	w.ShowAndRun()
}
