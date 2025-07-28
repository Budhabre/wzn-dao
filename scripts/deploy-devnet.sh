#!/bin/bash

# WZN Card Game Devnet Deployment Script
# This script deploys the WZN Card Game program to Solana Devnet

set -e

echo "🚀 Starting WZN Card Game Devnet Deployment"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Check prerequisites
echo -e "${BLUE}Checking prerequisites...${NC}"

# Check if Anchor is installed
if ! command -v anchor &> /dev/null; then
    echo -e "${RED}Error: Anchor CLI is not installed${NC}"
    echo "Please install Anchor: https://www.anchor-lang.com/docs/installation"
    exit 1
fi

# Check if Solana is installed
if ! command -v solana &> /dev/null; then
    echo -e "${RED}Error: Solana CLI is not installed${NC}"
    echo "Please install Solana: https://docs.solana.com/cli/install-solana-cli-tools"
    exit 1
fi

# Set Solana to devnet
echo -e "${YELLOW}Configuring Solana CLI for devnet...${NC}"
solana config set --url devnet

# Check wallet balance
BALANCE=$(solana balance --lamports)
MIN_BALANCE=5000000000  # 5 SOL in lamports

if [ "$BALANCE" -lt "$MIN_BALANCE" ]; then
    echo -e "${YELLOW}Insufficient balance. Requesting airdrop...${NC}"
    solana airdrop 5
    sleep 5
fi

echo -e "${GREEN}✓ Wallet balance: $(solana balance)${NC}"

# Build the program
echo -e "${BLUE}Building program...${NC}"
anchor build

# Deploy to devnet
echo -e "${BLUE}Deploying to devnet...${NC}"
PROGRAM_ID=$(anchor deploy --provider.cluster devnet | grep "Program Id:" | awk '{print $3}')

if [ -z "$PROGRAM_ID" ]; then
    echo -e "${RED}Error: Failed to deploy program${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Program deployed with ID: ${PROGRAM_ID}${NC}"

# Update program ID in lib.rs if different
echo -e "${BLUE}Updating program ID in source code...${NC}"
sed -i "s/Fg6PaFpoGXkYsidMaFRYvVpj6oH7wKq4WBZq2CFuXZJQ/${PROGRAM_ID}/g" programs/wzn_card_game/src/lib.rs

# Rebuild with correct program ID
echo -e "${BLUE}Rebuilding with correct program ID...${NC}"
anchor build

# Redeploy
echo -e "${BLUE}Redeploying with correct program ID...${NC}"
anchor deploy --provider.cluster devnet

# Create deployment info file
echo -e "${BLUE}Creating deployment info...${NC}"
cat > deployment-info.json << EOF
{
  "network": "devnet",
  "programId": "${PROGRAM_ID}",
  "deployedAt": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "cluster": "https://api.devnet.solana.com",
  "explorer": "https://explorer.solana.com/address/${PROGRAM_ID}?cluster=devnet",
  "rpcEndpoint": "https://api.devnet.solana.com",
  "wsEndpoint": "wss://api.devnet.solana.com/"
}
EOF

# Print deployment summary
echo -e "${GREEN}"
echo "=================================================="
echo "🎉 WZN Card Game Successfully Deployed to Devnet!"
echo "=================================================="
echo -e "${NC}"
echo -e "Program ID: ${GREEN}${PROGRAM_ID}${NC}"
echo -e "Network: ${BLUE}Devnet${NC}"
echo -e "Explorer: ${BLUE}https://explorer.solana.com/address/${PROGRAM_ID}?cluster=devnet${NC}"
echo -e "RPC Endpoint: ${BLUE}https://api.devnet.solana.com${NC}"
echo ""
echo -e "${YELLOW}Next Steps:${NC}"
echo "1. Initialize the global config and burn vault"
echo "2. Create WZN token mint and fund test accounts"
echo "3. Run integration tests against devnet"
echo ""
echo -e "${YELLOW}Initialize Commands:${NC}"
echo "anchor run initialize --provider.cluster devnet"
echo ""
echo -e "Deployment info saved to: ${GREEN}deployment-info.json${NC}"
echo ""
echo -e "${GREEN}✅ Deployment completed successfully!${NC}"
