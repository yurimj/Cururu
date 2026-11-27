# Gera apenas o executável e uma distribuição portátil limpa, nunca instalador.
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
$raiz = (Get-Location).Path
$versao = (Get-Content package.json -Raw | ConvertFrom-Json).version
$runtimeVersao = '158.0.4370.33'
$cab = Join-Path $raiz "runtime/webview2-$runtimeVersao-x64.cab"
$extraido = Join-Path $raiz 'runtime/webview2-extraido'
$runtime = Join-Path $extraido "Microsoft.WebView2.FixedVersionRuntime.$runtimeVersao.x64"
if (!(Test-Path "$runtime/msedgewebview2.exe")) {
    New-Item -ItemType Directory -Force $extraido | Out-Null
    if (!(Test-Path $cab)) {
        & curl.exe -L --fail --silent --show-error 'https://msedge.sf.dl.delivery.mp.microsoft.com/filestreamingservice/files/ecc2761e-1380-46ee-9552-995b039e4e47/Microsoft.WebView2.FixedVersionRuntime.158.0.4370.33.x64.cab' -o $cab
        if ($LASTEXITCODE -ne 0) { throw 'Falha ao baixar o WebView2 oficial.' }
    }
    & expand.exe $cab '-F:*' $extraido | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Falha ao extrair o WebView2.' }
}
$assinatura = Get-AuthenticodeSignature "$runtime/msedgewebview2.exe"
if ($assinatura.Status -ne 'Valid' -or $assinatura.SignerCertificate.Subject -notmatch 'O=Microsoft Corporation') {
    throw 'A assinatura Microsoft do WebView2 não foi validada.'
}
$release = Join-Path $raiz 'src-tauri/target/release'
$exe = Join-Path $release 'cururu.exe'
# Somente backups deste executável, dentro do diretório release conhecido.
Get-ChildItem -LiteralPath $release -File | Where-Object { $_.Name -eq 'cururu.exe.old' -or $_.Name -like 'cururu.exe.*.old' } | ForEach-Object {
    Remove-Item -LiteralPath $_.FullName -ErrorAction SilentlyContinue
}
$backup = $null
if (Get-Process cururu -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $exe }) {
    $backup = "$exe.$(Get-Date -Format yyyyMMdd-HHmmss).old"
    Move-Item -LiteralPath $exe -Destination $backup
}
try {
    & npm.cmd run tauri build -- --no-bundle
    if ($LASTEXITCODE -ne 0) { throw 'Falha ao compilar o Cururu.' }
} catch {
    if ($backup -and !(Test-Path $exe)) { Move-Item -LiteralPath $backup -Destination $exe }
    throw
}
$webviewDestino = Join-Path $release 'webview2'
New-Item -ItemType Directory -Force $webviewDestino | Out-Null
Get-ChildItem -LiteralPath $runtime | Copy-Item -Destination $webviewDestino -Recurse -Force
if (!(Test-Path "$release/instantclient/oci.dll")) {
    Copy-Item -LiteralPath 'runtime/instantclient_19_31' -Destination "$release/instantclient" -Recurse
}
# O diretório de entrega é novo: nunca copiamos o release inteiro, logs ou perfis.
$entrega = Join-Path $raiz "src-tauri/target/portatil/Cururu-$versao"
if (Test-Path $entrega) { throw "A entrega já existe: $entrega" }
$pasta = Join-Path $entrega 'Cururu'
New-Item -ItemType Directory -Force $pasta | Out-Null
Copy-Item -LiteralPath $exe -Destination $pasta
Copy-Item -LiteralPath "$release/instantclient" -Destination $pasta -Recurse
Copy-Item -LiteralPath "$release/webview2" -Destination $pasta -Recurse
Copy-Item -LiteralPath 'LICENSE','LICENCA.md','TERMOS-ADICIONAIS.txt' -Destination $pasta
@"
Cururu $versao - Windows x64

Extraia a pasta Cururu inteira para uma pasta local e abra cururu.exe.
Mantenha instantclient e webview2 junto do executável. Não execute dentro do ZIP
nem de um compartilhamento de rede. Não é preciso instalar o WebView2.

O runtime WebView2 $runtimeVersao é distribuído sem modificações e não recebe
atualizações automáticas nesta edição. Atualize a pasta portátil completa.
Conexões e preferências são guardadas no perfil do Windows de quem usa o programa.

Código-fonte: https://github.com/yurimj/Cururu
"@ | Set-Content -LiteralPath "$pasta/LEIA-ME.txt" -Encoding utf8
$zip = Join-Path (Split-Path $entrega -Parent) "Cururu-$versao-portatil-x64.zip"
Compress-Archive -LiteralPath $pasta -DestinationPath $zip -CompressionLevel Optimal
Get-FileHash -LiteralPath $zip -Algorithm SHA256
Write-Output "Portátil pronto: $zip"
