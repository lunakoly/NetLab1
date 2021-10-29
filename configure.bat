@echo off

set HERE="%~dp0"

mklink /d "%HERE%\client\target" "..\target"
mklink /d "%HERE%\server\target" "..\target"
