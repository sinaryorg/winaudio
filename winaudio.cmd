@echo off
"%~dp0target\release\winaudio.exe" %*
if "%~1"=="" pause
