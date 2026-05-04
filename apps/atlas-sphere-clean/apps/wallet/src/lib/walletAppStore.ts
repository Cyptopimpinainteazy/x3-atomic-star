/**
 * Wallet App Integration
 * 
 * Connects the standalone wallet app with the existing wallet store.
 * Provides seamless sync between desktop and web wallet interfaces.
 */

import { useEffect, useCallback } from 'react';
import { useWalletStore, type WalletState } from '@/stores/walletStore';
import { walletService } from '@/services/walletService';

// ============================================================================
// Wallet App Store Integration Hook
// ============================================================================

export function useWalletAppStore() {
  const {
    // State
    isConnected,
    isLoading,
    accounts,
    activeAccountIndex,
    totalBalance,
    tokens,
    transactions,
    pendingComits,
    activeView,
    evmChainCount,
    universalWallet,
    addressBook,
    portfolioTokens,
    comits,
    gpuEarningEnabled,
    cpuEarningEnabled,
    phoneEarningEnabled,
    storageContributionEnabled,
    
    // Actions
    setConnected,
    setLoading,
    addAccount,
    setAccounts,
    setActiveAccountIndex,
    setActiveView,
    setTokens,
    addTransaction,
    disconnect,
    setUniversalWallet,
    setEvmChainCount,
    addContact,
    addPortfolioToken,
    addComit,
    setGpuEarning,
    setCpuEarning,
    setPhoneEarning,
    setStorageContribution,
    generateWallet,
    importWallet,
  } = useWalletStore();

  // Sync wallet state with backend
  const syncWalletState = useCallback(async () => {
    if (!universalWallet) return;

    try {
      setLoading(true);
      
      // Get wallet status from backend
      const status = await walletService.getWalletStatus({
        walletId: universalWallet.evm_address,
      });

      // Update local state with backend data
      setConnected(status.status.isConnected);
      setEvmChainCount(status.status.network === 'mainnet' ? 1 : 0);
      
      // Get balance from backend
      const balance = await walletService.getBalance({
        walletId: universalWallet.evm_address,
        network: status.status.network,
      });

      setTokens(balance.tokens);

      // Get transactions from backend
      const history = await walletService.getTransactions({
        walletId: universalWallet.evm_address,
        network: status.status.network,
      });

      // Merge transactions (keep local ones that aren't in backend)
      const existingHashes = transactions.map(t => t.hash);
      const newTransactions = history.transactions
        .filter(tx => !existingHashes.includes(tx.hash))
        .map(tx => ({
          id: tx.hash,
          type: tx.hash.startsWith('0x') ? 'send' : 'receive',
          status: tx.status as any,
          amount: parseFloat(tx.amount) / 1e18,
          symbol: tx.token,
          from: tx.from,
          to: tx.to,
          timestamp: tx.timestamp,
          hash: tx.hash,
          network: 'evm' as const,
        }));

      newTransactions.forEach(addTransaction);

    } catch (error) {
      console.error('Failed to sync wallet state:', error);
    } finally {
      setLoading(false);
    }
  }, [universalWallet, setConnected, setLoading, setEvmChainCount, setTokens, addTransaction, transactions]);

  // Auto-sync when connected
  useEffect(() => {
    if (isConnected && universalWallet) {
      syncWalletState();
      
      // Set up periodic sync every 30 seconds
      const interval = setInterval(syncWalletState, 30000);
      return () => clearInterval(interval);
    }
  }, [isConnected, universalWallet, syncWalletState]);

  // Handle wallet connection
  const connectWallet = useCallback(async () => {
    if (universalWallet) {
      setConnected(true);
      await syncWalletState();
    } else {
      await generateWallet();
    }
  }, [universalWallet, setConnected, syncWalletState, generateWallet]);

  // Handle wallet disconnection
  const disconnectWallet = useCallback(() => {
    disconnect();
    setConnected(false);
  }, [disconnect, setConnected]);

  // Handle wallet import
  const importExistingWallet = useCallback(async (mnemonic: string) => {
    try {
      await importWallet(mnemonic);
      setConnected(true);
      await syncWalletState();
    } catch (error) {
      console.error('Failed to import wallet:', error);
      throw error;
    }
  }, [importWallet, setConnected, syncWalletState]);

  // Handle transaction signing
  const signTransaction = useCallback(async (
    transactionData: string,
    network: string = 'mainnet'
  ) => {
    if (!universalWallet) {
      throw new Error('Wallet not connected');
    }

    return await walletService.signTransaction({
      walletId: universalWallet.evm_address,
      passwordHash: universalWallet.seed_hex,
      transactionData,
      network,
    });
  }, [universalWallet]);

  // Handle transaction submission
  const submitTransaction = useCallback(async (
    signedTransaction: string,
    network: string = 'mainnet'
  ) => {
    return await walletService.submitTransaction({
      signedTransaction,
      network,
    });
  }, []);

  // Handle network switching
  const switchNetwork = useCallback(async (network: string) => {
    if (!universalWallet) {
      throw new Error('Wallet not connected');
    }

    const response = await walletService.setNetwork({
      walletId: universalWallet.evm_address,
      network,
    });

    if (response.success) {
      await syncWalletState();
    }

    return response;
  }, [universalWallet, syncWalletState]);

  // Get available networks
  const getAvailableNetworks = useCallback(async () => {
    return await walletService.getNetworks();
  }, []);

  return {
    // State
    isConnected,
    isLoading,
    accounts,
    activeAccountIndex,
    totalBalance,
    tokens,
    transactions,
    pendingComits,
    activeView,
    evmChainCount,
    universalWallet,
    addressBook,
    portfolioTokens,
    comits,
    gpuEarningEnabled,
    cpuEarningEnabled,
    phoneEarningEnabled,
    storageContributionEnabled,

    // Actions
    setConnected,
    setLoading,
    addAccount,
    setAccounts,
    setActiveAccountIndex,
    setActiveView,
    setTokens,
    addTransaction,
    disconnect,
    setUniversalWallet,
    setEvmChainCount,
    addContact,
    addPortfolioToken,
    addComit,
    setGpuEarning,
    setCpuEarning,
    setPhoneEarning,
    setStorageContribution,
    generateWallet,
    importWallet,

    // App-specific actions
    connectWallet,
    disconnectWallet,
    importExistingWallet,
    signTransaction,
    submitTransaction,
    switchNetwork,
    getAvailableNetworks,
    syncWalletState,
  };
}

// ============================================================================
// Wallet App Provider Component
// ============================================================================

interface WalletAppProviderProps {
  children: React.ReactNode;
}

export function WalletAppProvider({ children }: WalletAppProviderProps) {
  // The hook is already connected to Zustand store
  // This component just provides the context if needed
  return <>{children}</>;
}

// ============================================================================
// Wallet App Hooks
// ============================================================================

export function useWalletBalance() {
  const { tokens, totalBalance } = useWalletStore();
  return { tokens, totalBalance };
}

export function useWalletTransactions() {
  const { transactions, addTransaction } = useWalletStore();
  return { transactions, addTransaction };
}

export function useWalletActions() {
  const { generateWallet, importWallet, disconnect, signTransaction, submitTransaction } = useWalletStore();
  return { generateWallet, importWallet, disconnect, signTransaction, submitTransaction };
}
