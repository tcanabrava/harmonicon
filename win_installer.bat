@echo off
setlocal

@REM compila o build.rs para gerar os artefados wix/assets.wxi
rustc build.rs -o build_script.exe

REM Se o primeiro parâmetro foi informado, usa ele como caminho do ISCC.exe
if not "%~1"=="" (
    set "ISCC=%~1"
) else (
    REM Senão tenta encontrar no PATH
    where ISCC >nul 2>&1
    if errorlevel 1 (
        echo Erro: ISCC.exe nao encontrado.
        echo Uso:
        echo    installer.bat "C:\Program Files (x86)\Inno Setup 7\ISCC.exe"
        echo Ou adicione o diretorio do Inno Setup ao PATH.
        exit /b 1
    )
    set "ISCC=ISCC"
)

echo Usando: %ISCC%
"%ISCC%" packaging\windows\setup.iss

if errorlevel 1 (
    echo Falha ao gerar o instalador.
    exit /b 1
)

echo Instalador gerado com sucesso.
exit /b 0