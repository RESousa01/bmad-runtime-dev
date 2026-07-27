; Forge's replacement for Tauri's bundled NSIS English message file.
;
; Wired up through tauri.conf.json > bundle > windows > nsis >
; customLanguageFiles. Supplying a file for a language REPLACES Tauri's file
; for that language outright -- there is no partial override -- so every
; LangString the generated installer.nsi references must be defined here or
; the NSIS build fails with an undefined language string. The exact name set
; is pinned by tools/forge-brand.test.mjs so a Tauri upgrade that adds a
; message is caught in the source lane rather than in a release build.
;
; Only NSIS define substitution is used here, never a handlebars placeholder.
; Tauri's own file mixes the two; ${PRODUCTNAME} is correct either way because
; it is a plain define emitted at the top of the generated script.
;
; Keep this file ASCII-only. The generated script is Unicode, and an included
; file with no byte-order mark is read as ANSI -- identical for ASCII.

; --- Reinstall and maintenance page ----------------------------------------

LangString addOrReinstall ${LANG_ENGLISH} "Add or reinstall components"
LangString alreadyInstalled ${LANG_ENGLISH} "Already installed"
LangString alreadyInstalledLong ${LANG_ENGLISH} "${PRODUCTNAME} ${VERSION} is already installed on this device. Choose what you want to do and click Next to continue."
LangString chooseMaintenanceOption ${LANG_ENGLISH} "Choose what you want to do."
LangString choowHowToInstall ${LANG_ENGLISH} "Choose how you want to install ${PRODUCTNAME}."
LangString newerVersionInstalled ${LANG_ENGLISH} "A newer version of ${PRODUCTNAME} is already installed. Installing an older version over it is not recommended; uninstall the current version first if you need to. Choose what you want to do and click Next to continue."
LangString older ${LANG_ENGLISH} "older"
LangString olderOrUnknownVersionInstalled ${LANG_ENGLISH} "An $R4 version of ${PRODUCTNAME} is installed on this device. Uninstalling it before continuing is recommended. Choose what you want to do and click Next to continue."
LangString unknown ${LANG_ENGLISH} "unknown"
LangString uninstallBeforeInstalling ${LANG_ENGLISH} "Uninstall before installing"
LangString dontUninstall ${LANG_ENGLISH} "Do not uninstall"
LangString dontUninstallDowngrade ${LANG_ENGLISH} "Do not uninstall (not allowed when installing an older version)"
LangString silentDowngrades ${LANG_ENGLISH} "This installer does not install an older version silently. Run it normally instead.$\n"

; --- A running copy of Forge blocks install and uninstall ------------------

LangString appRunning ${LANG_ENGLISH} "${PRODUCTNAME} is still running. Close it and try again."
LangString appRunningOkKill ${LANG_ENGLISH} "${PRODUCTNAME} is still running.$\nClick OK to close it and continue."
LangString failedToKillApp ${LANG_ENGLISH} "${PRODUCTNAME} could not be closed automatically. Close it and try again."

; --- Shortcuts and the WebView2 runtime ------------------------------------
;
; Forge bundles the WebView2 runtime inside the installer (webviewInstallMode
; offlineInstaller) so that it installs with no internet connection. The
; download messages below are therefore unreachable in a normal Forge build,
; but the generated script still references them.

LangString createDesktop ${LANG_ENGLISH} "Create a desktop shortcut"
LangString installingWebview2 ${LANG_ENGLISH} "Installing the Microsoft Edge WebView2 runtime..."
LangString webview2AbortError ${LANG_ENGLISH} "The Microsoft Edge WebView2 runtime could not be installed. ${PRODUCTNAME} cannot run without it. Try running this installer again."
LangString webview2DownloadError ${LANG_ENGLISH} "Error: the WebView2 runtime could not be downloaded - $0"
LangString webview2DownloadSuccess ${LANG_ENGLISH} "WebView2 runtime downloaded"
LangString webview2Downloading ${LANG_ENGLISH} "Downloading the WebView2 runtime..."
LangString webview2InstallError ${LANG_ENGLISH} "Error: installing the WebView2 runtime failed with exit code $1"
LangString webview2InstallSuccess ${LANG_ENGLISH} "WebView2 runtime installed"

; --- Uninstaller -----------------------------------------------------------
;
; deleteAppData labels the checkbox on the uninstall confirmation page. The
; stock text ("Delete the application data") does not say what is destroyed,
; and what is destroyed here is the encrypted local authority store: chat
; transcripts, approval decisions, capability results, and every workspace
; grant. The label states the stakes; installer/hooks.nsh asks once more and
; enumerates the contents before anything is deleted.

LangString uninstallApp ${LANG_ENGLISH} "Uninstall ${PRODUCTNAME}"
LangString unableToUninstall ${LANG_ENGLISH} "${PRODUCTNAME} could not be uninstalled."
LangString deleteAppData ${LANG_ENGLISH} "Also erase ${PRODUCTNAME}'s local data (cannot be undone)"

; --- Forge's own messages --------------------------------------------------

LangString forgeEraseLocalDataConfirm ${LANG_ENGLISH} "You chose to erase ${PRODUCTNAME}'s local data.$\n$\nThis permanently deletes the conversations, approval decisions, capability results, and workspace folder grants that ${PRODUCTNAME} keeps on this device. It cannot be undone and ${PRODUCTNAME} cannot recover it.$\n$\nYour own workspace files are not touched.$\n$\nErase the local data?"
