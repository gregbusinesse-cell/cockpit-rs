# Publie une nouvelle version de Cockpit (version Zest) : Noam et les autres la reçoivent à l'ouverture ou en appuyant sur ↻.
#   powershell -ExecutionPolicy Bypass -File publier.ps1
# Monte la version (0.1.1 -> 0.1.2), compile, signe, crée la release GitHub avec l'installeur et latest.json.
$ErrorActionPreference = "Stop"
$racine = $PSScriptRoot
$conf = Join-Path $racine "crates\app\tauri.conf.json"
$cles = "C:\Users\gregd\DISQUE_DUR_A_TRANSFERER\archives\cockpit_donnees_19nov\dot_cockpit\updater"

$json = Get-Content $conf -Raw -Encoding UTF8 | ConvertFrom-Json
$v = [version]$json.version
$nouvelle = "{0}.{1}.{2}" -f $v.Major, $v.Minor, ($v.Build + 1)
if ($args -contains "--meme-version") { $nouvelle = $json.version }
$texte = (Get-Content $conf -Raw -Encoding UTF8) -replace ('"version": "' + [regex]::Escape($json.version) + '"'), ('"version": "' + $nouvelle + '"')
[IO.File]::WriteAllText($conf, $texte, (New-Object System.Text.UTF8Encoding $false))
Write-Host "Version $nouvelle"

$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content (Join-Path $cles "cockpit.key") -Raw
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = (Get-Content (Join-Path $cles "motdepasse.txt") -Raw).Trim()
$vcvars = "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat"
cmd /c "`"$vcvars`" >nul && cd /d `"$racine\crates\app`" && cargo tauri build --bundles nsis"
if ($LASTEXITCODE -ne 0) { throw "compilation ratée" }

$nsis = Join-Path $racine "target\release\bundle\nsis"
$setup = Join-Path $nsis "Cockpit_${nouvelle}_x64-setup.exe"
$sig = (Get-Content "$setup.sig" -Raw).Trim()
$copie = Join-Path $nsis "Cockpit-setup.exe"
Copy-Item $setup $copie -Force
$latest = [ordered]@{
  version = $nouvelle
  notes = "Cockpit $nouvelle"
  pub_date = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
  platforms = @{ "windows-x86_64" = @{ signature = $sig; url = "https://github.com/gregbusinesse-cell/cockpit-rs/releases/download/v$nouvelle/Cockpit_${nouvelle}_x64-setup.exe" } }
}
$latestPath = Join-Path $nsis "latest.json"
[IO.File]::WriteAllText($latestPath, ($latest | ConvertTo-Json -Depth 5), (New-Object System.Text.UTF8Encoding $false))

git -C $racine add -A
git -C $racine commit -m "Cockpit $nouvelle" | Out-Null
git -C $racine push greg zest
gh release create "v$nouvelle" $setup $copie $latestPath -R gregbusinesse-cell/cockpit-rs --target zest --title "Cockpit $nouvelle" --notes "Mise à jour automatique."
Write-Host "Publié. Lien d'installation : https://github.com/gregbusinesse-cell/cockpit-rs/releases/latest/download/Cockpit-setup.exe"
