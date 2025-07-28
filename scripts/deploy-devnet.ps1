# WZN Card Game Devnet Deployment Script (PowerShell)
# This script deploys the WZN Card Game program to Solana Devnet

param(
    [switch]$SkipBuild = $false,
    [switch]$Force = $false
)

Write-Host "🚀 Starting WZN Card Game Devnet Deployment" -ForegroundColor Cyan

# Check prerequisites
Write-Host "Checking prerequisites..." -ForegroundColor Blue

# Check if Anchor is installed
try {
    $anchorVersion = anchor --version
    Write-Host "✓ Anchor CLI found: $anchorVersion" -ForegroundColor Green
} catch {
    Write-Host "❌ Error: Anchor CLI is not installed" -ForegroundColor Red
    Write-Host "Please install Anchor: https://www.anchor-lang.com/docs/installation"
    exit 1
}

# Check if Solana is installed  
try {
    $solanaVersion = solana --version
    Write-Host "✓ Solana CLI found: $solanaVersion" -ForegroundColor Green
} catch {
    Write-Host "❌ Error: Solana CLI is not installed" -ForegroundColor Red
    Write-Host "Please install Solana: https://docs.solana.com/cli/install-solana-cli-tools"
    exit 1
}

# Set Solana to devnet
Write-Host "Configuring Solana CLI for devnet..." -ForegroundColor Yellow
solana config set --url devnet

# Check wallet balance
$balance = (solana balance --lamports) -as [long]
$minBalance = 5000000000  # 5 SOL in lamports

if ($balance -lt $minBalance) {
    Write-Host "Insufficient balance. Requesting airdrop..." -ForegroundColor Yellow
    solana airdrop 5
    Start-Sleep -Seconds 5
}

$currentBalance = solana balance
Write-Host "✓ Wallet balance: $currentBalance" -ForegroundColor Green

# Build the program
if (-not $SkipBuild) {
    Write-Host "Building program..." -ForegroundColor Blue
    anchor build
    
    if ($LASTEXITCODE -ne 0) {
        Write-Host "❌ Build failed" -ForegroundColor Red
        exit 1
    }
}

# Deploy to devnet
Write-Host "Deploying to devnet..." -ForegroundColor Blue
$deployOutput = anchor deploy --provider.cluster devnet 2>&1
$programId = ($deployOutput | Select-String "Program Id: (.+)" | ForEach-Object { $_.Matches[0].Groups[1].Value })

if (-not $programId) {
    Write-Host "❌ Failed to extract program ID from deployment output" -ForegroundColor Red
    Write-Host "Deployment output:" -ForegroundColor Yellow
    Write-Host $deployOutput
    exit 1
}

Write-Host "✓ Program deployed with ID: $programId" -ForegroundColor Green

# Update program ID in lib.rs if different
$libPath = "programs\wzn_card_game\src\lib.rs"
$currentContent = Get-Content $libPath -Raw
$oldProgramId = "Fg6PaFpoGXkYsidMaFRYvVpj6oH7wKq4WBZq2CFuXZJQ"

if ($currentContent -match $oldProgramId -and $programId -ne $oldProgramId) {
    Write-Host "Updating program ID in source code..." -ForegroundColor Blue
    $newContent = $currentContent -replace $oldProgramId, $programId
    Set-Content -Path $libPath -Value $newContent
    
    # Rebuild with correct program ID
    Write-Host "Rebuilding with correct program ID..." -ForegroundColor Blue
    anchor build
    
    # Redeploy
    Write-Host "Redeploying with correct program ID..." -ForegroundColor Blue
    anchor deploy --provider.cluster devnet
}

# Create deployment info file
Write-Host "Creating deployment info..." -ForegroundColor Blue
$deploymentInfo = @{
    network = "devnet"
    programId = $programId
    deployedAt = (Get-Date -Format "yyyy-MM-ddTHH:mm:ssZ")
    cluster = "https://api.devnet.solana.com"
    explorer = "https://explorer.solana.com/address/$programId?cluster=devnet"
    rpcEndpoint = "https://api.devnet.solana.com"
    wsEndpoint = "wss://api.devnet.solana.com/"
} | ConvertTo-Json -Depth 10

$deploymentInfo | Out-File -FilePath "deployment-info.json" -Encoding UTF8

# Print deployment summary
Write-Host ""
Write-Host "==================================================" -ForegroundColor Green
Write-Host "🎉 WZN Card Game Successfully Deployed to Devnet!" -ForegroundColor Green
Write-Host "==================================================" -ForegroundColor Green
Write-Host ""
Write-Host "Program ID: " -NoNewline
Write-Host $programId -ForegroundColor Green
Write-Host "Network: " -NoNewline  
Write-Host "Devnet" -ForegroundColor Blue
Write-Host "Explorer: " -NoNewline
Write-Host "https://explorer.solana.com/address/$programId?cluster=devnet" -ForegroundColor Blue
Write-Host "RPC Endpoint: " -NoNewline
Write-Host "https://api.devnet.solana.com" -ForegroundColor Blue
Write-Host ""
Write-Host "Next Steps:" -ForegroundColor Yellow
Write-Host "1. Initialize the global config and burn vault"
Write-Host "2. Create WZN token mint and fund test accounts"  
Write-Host "3. Run integration tests against devnet"
Write-Host ""
Write-Host "Initialize Commands:" -ForegroundColor Yellow
Write-Host "anchor run initialize --provider.cluster devnet"
Write-Host ""
Write-Host "Deployment info saved to: " -NoNewline
Write-Host "deployment-info.json" -ForegroundColor Green
Write-Host ""
Write-Host "✅ Deployment completed successfully!" -ForegroundColor Green
