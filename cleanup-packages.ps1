# WZN Card Game - Package Cleanup Script
# This script helps you clean up various package caches to free disk space

Write-Host "=== Package Cleanup Script ===" -ForegroundColor Cyan
Write-Host ""

# Function to get directory size
function Get-DirectorySize($path) {
    if (Test-Path $path) {
        $size = Get-ChildItem -Path $path -Recurse -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum
        return [math]::Round($size.Sum / 1MB, 2)
    }
    return 0
}

# Show current sizes
Write-Host "Current package sizes:" -ForegroundColor Yellow
Write-Host "1. Cargo cache: $(Get-DirectorySize 'C:\Users\vladi\.cargo') MB"
Write-Host "2. NPM global cache: $(Get-DirectorySize 'C:\Users\vladi\AppData\Roaming\npm') MB"
Write-Host "3. NPM local cache: $(Get-DirectorySize 'C:\Users\vladi\AppData\Local\npm-cache') MB"
Write-Host "4. Project target: $(Get-DirectorySize 'C:\Users\vladi\Documents\Project\upwork\wzn-card-game\target') MB"
Write-Host "5. Rustup toolchains: $(Get-DirectorySize 'C:\Users\vladi\.rustup') MB"
Write-Host ""

Write-Host "Choose what to clean:" -ForegroundColor Green
Write-Host "1 - Clean Cargo cache (safe - will redownload crates when needed)"
Write-Host "2 - Clean NPM caches (safe - will redownload packages when needed)"
Write-Host "3 - Clean project target directory (safe - will rebuild when needed)"
Write-Host "4 - Clean ALL except Rustup toolchains (recommended)"
Write-Host "5 - Clean EVERYTHING including Rustup (will need to reinstall Rust!)"
Write-Host "6 - Exit without cleaning"
Write-Host ""

$choice = Read-Host "Enter your choice (1-6)"

switch ($choice) {
    "1" {
        Write-Host "Cleaning Cargo cache..." -ForegroundColor Yellow
        cargo clean --manifest-path "C:\Users\vladi\Documents\Project\upwork\wzn-card-game\Cargo.toml"
        Remove-Item -Path "C:\Users\vladi\.cargo\registry" -Recurse -Force -ErrorAction SilentlyContinue
        Remove-Item -Path "C:\Users\vladi\.cargo\git" -Recurse -Force -ErrorAction SilentlyContinue
        Write-Host "Cargo cache cleaned!" -ForegroundColor Green
    }
    "2" {
        Write-Host "Cleaning NPM caches..." -ForegroundColor Yellow
        npm cache clean --force
        Remove-Item -Path "C:\Users\vladi\AppData\Local\npm-cache" -Recurse -Force -ErrorAction SilentlyContinue
        Write-Host "NPM caches cleaned!" -ForegroundColor Green
    }
    "3" {
        Write-Host "Cleaning project target directory..." -ForegroundColor Yellow
        Remove-Item -Path "C:\Users\vladi\Documents\Project\upwork\wzn-card-game\target" -Recurse -Force -ErrorAction SilentlyContinue
        Write-Host "Project target directory cleaned!" -ForegroundColor Green
    }
    "4" {
        Write-Host "Cleaning all caches except Rustup..." -ForegroundColor Yellow
        # Cargo
        cargo clean --manifest-path "C:\Users\vladi\Documents\Project\upwork\wzn-card-game\Cargo.toml"
        Remove-Item -Path "C:\Users\vladi\.cargo\registry" -Recurse -Force -ErrorAction SilentlyContinue
        Remove-Item -Path "C:\Users\vladi\.cargo\git" -Recurse -Force -ErrorAction SilentlyContinue
        # NPM
        npm cache clean --force
        Remove-Item -Path "C:\Users\vladi\AppData\Local\npm-cache" -Recurse -Force -ErrorAction SilentlyContinue
        # Project target
        Remove-Item -Path "C:\Users\vladi\Documents\Project\upwork\wzn-card-game\target" -Recurse -Force -ErrorAction SilentlyContinue
        Write-Host "All caches cleaned (except Rustup)!" -ForegroundColor Green
    }
    "5" {
        Write-Host "WARNING: This will remove Rust toolchains! You'll need to reinstall Rust!" -ForegroundColor Red
        $confirm = Read-Host "Type 'YES' to confirm"
        if ($confirm -eq "YES") {
            Write-Host "Cleaning EVERYTHING..." -ForegroundColor Yellow
            # All previous + Rustup
            cargo clean --manifest-path "C:\Users\vladi\Documents\Project\upwork\wzn-card-game\Cargo.toml"
            Remove-Item -Path "C:\Users\vladi\.cargo" -Recurse -Force -ErrorAction SilentlyContinue
            Remove-Item -Path "C:\Users\vladi\.rustup" -Recurse -Force -ErrorAction SilentlyContinue
            npm cache clean --force
            Remove-Item -Path "C:\Users\vladi\AppData\Local\npm-cache" -Recurse -Force -ErrorAction SilentlyContinue
            Remove-Item -Path "C:\Users\vladi\Documents\Project\upwork\wzn-card-game\target" -Recurse -Force -ErrorAction SilentlyContinue
            Write-Host "Everything cleaned! You'll need to reinstall Rust and Anchor." -ForegroundColor Green
        } else {
            Write-Host "Cancelled." -ForegroundColor Yellow
        }
    }
    "6" {
        Write-Host "Exiting without cleaning." -ForegroundColor Yellow
        exit
    }
    default {
        Write-Host "Invalid choice. Exiting." -ForegroundColor Red
        exit
    }
}

Write-Host ""
Write-Host "Cleanup complete! New sizes:" -ForegroundColor Cyan
Write-Host "1. Cargo cache: $(Get-DirectorySize 'C:\Users\vladi\.cargo') MB"
Write-Host "2. NPM global cache: $(Get-DirectorySize 'C:\Users\vladi\AppData\Roaming\npm') MB"
Write-Host "3. NPM local cache: $(Get-DirectorySize 'C:\Users\vladi\AppData\Local\npm-cache') MB"
Write-Host "4. Project target: $(Get-DirectorySize 'C:\Users\vladi\Documents\Project\upwork\wzn-card-game\target') MB"
Write-Host "5. Rustup toolchains: $(Get-DirectorySize 'C:\Users\vladi\.rustup') MB"
