/**
 * Phase 3 Wallet Panels - Enhanced Wallet UI Components
 * 
 * This module provides comprehensive wallet management UI components including:
 * - WalletDashboardPanel: Main wallet dashboard with balance and quick actions
 * - WalletTransactionsPanel: Transaction history and management
 * - WalletSettingsPanel: Wallet configuration and security settings
 * - NetworkSelectorPanel: Network selection (mainnet/testnet/local)
 * - WalletSecurityPanel: Security features and PIN protection
 */

import React, { useState, useEffect } from 'react';
import { 
  LayoutDashboard, Send, ArrowDownLeft, ArrowLeftRight, History, Settings, 
  Shield, Lock, Key, Globe, ChevronDown, Plus, Search, Filter,
  Clock, CheckCircle, AlertCircle, Loader2, Copy, QrCode, Download,
  Trash2, Eye, EyeOff, Smartphone, Fingerprint, ShieldCheck, LockKeyhole
} from 'lucide-react';
import { useWalletStore } from '@/stores/walletStore';

const formatAddress = (address: string) => {
  if (address.length <= 12) return address;
  return `${address.slice(0, 6)}...${address.slice(-4)}`;
};

// ============================================================================
// Types and Interfaces
// ============================================================================

interface WalletPanelProps {
  className?: string;
}

interface Transaction {
  id: string;
  type: 'send' | 'receive' | 'swap' | 'comit' | 'mint';
  status: 'confirmed' | 'pending' | 'failed';
  amount: number;
  symbol: string;
  from: string;
  to: string;
  timestamp: number;
  hash: string;
  network: 'evm' | 'svm' | 'substrate';
  comitId?: string;
  blockNumber?: number;
}

interface Token {
  symbol: string;
  name: string;
  balance: number;
  value: number;
  change24h: number;
  icon: string;
  network: 'evm' | 'svm' | 'substrate';
  color?: string;
}

// ============================================================================
// Network Configuration
// ============================================================================

const NETWORKS = [
  { id: 'mainnet', name: 'X3 Mainnet', chainId: 123456789, rpc: 'https://rpc.x3chain.io', isTestnet: false },
  { id: 'testnet', name: 'X3 Testnet', chainId: 123456788, rpc: 'https://rpc-testnet.x3chain.io', isTestnet: true },
  { id: 'local', name: 'Local Devnet', chainId: 123456787, rpc: 'http://localhost:9933', isTestnet: true },
] as const;

type NetworkId = typeof NETWORKS[number]['id'];

// ============================================================================
// Network Selector Panel
// ============================================================================

export function NetworkSelectorPanel({ className }: WalletPanelProps) {
  const [selectedNetwork, setSelectedNetwork] = useState<NetworkId>('mainnet');
  const [isOpen, setIsOpen] = useState(false);

  const network = NETWORKS.find(n => n.id === selectedNetwork) || NETWORKS[0];

  return (
    <div className={`relative ${className}`}>
      <button
        onClick={() => setIsOpen(!isOpen)}
        className="flex items-center gap-2 px-4 py-2 bg-[#1a1a2e] border border-[#2a2a35] rounded-lg hover:border-[#3a3a45] transition-colors"
      >
        <Globe className="w-4 h-4 text-blue-400" />
        <span className="text-sm font-medium text-white">{network.name}</span>
        <ChevronDown className={`w-4 h-4 text-gray-400 transition-transform ${isOpen ? 'rotate-180' : ''}`} />
      </button>

      {isOpen && (
        <>
          <div className="fixed inset-0 z-10" onClick={() => setIsOpen(false)} />
          <div className="absolute top-full left-0 mt-2 w-64 z-20 bg-[#0a0a0f] border border-[#2a2a35] rounded-xl shadow-2xl overflow-hidden">
            <div className="p-2">
              {NETWORKS.map((net) => (
                <button
                  key={net.id}
                  onClick={() => {
                    setSelectedNetwork(net.id as NetworkId);
                    setIsOpen(false);
                  }}
                  className={`w-full flex items-center gap-3 px-3 py-2.5 rounded-lg transition-colors ${
                    selectedNetwork === net.id
                      ? 'bg-blue-500/20 text-blue-400'
                      : 'hover:bg-[#1a1a2e] text-gray-300'
                  }`}
                >
                  <Globe className={`w-4 h-4 ${net.isTestnet ? 'text-yellow-400' : 'text-green-400'}`} />
                  <div className="flex-1 text-left">
                    <div className="text-sm font-medium">{net.name}</div>
                    <div className="text-xs opacity-60">{net.rpc}</div>
                  </div>
                  {selectedNetwork === net.id && <CheckCircle className="w-4 h-4" />}
                </button>
              ))}
            </div>
          </div>
        </>
      )}
    </div>
  );
}

// ============================================================================
// Wallet Dashboard Panel
// ============================================================================

export function WalletDashboardPanel({ className }: WalletPanelProps) {
  const { tokens = [], transactions = [], totalBalance, isConnected } = useWalletStore();
  const [isLoading, setIsLoading] = useState(false);

  const refreshBalances = () => {
    setIsLoading(true);
    setTimeout(() => setIsLoading(false), 1000);
  };

  return (
    <div className={`space-y-6 ${className}`}>
      {/* Header with Network Selector */}
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold text-white">Wallet Dashboard</h2>
          <p className="text-sm text-gray-400">Manage your multi-chain assets</p>
        </div>
        <NetworkSelectorPanel />
      </div>

      {/* Balance Card */}
      <div className="bg-gradient-to-br from-[#111] via-[#151515] to-[#0a0a0f] border border-[#222] p-6 rounded-3xl shadow-2xl relative overflow-hidden">
        <div className="absolute top-0 right-0 p-8 opacity-20 pointer-events-none">
          <Globe className="w-48 h-48 text-blue-500/20 animate-spin-slow" />
        </div>
        
        <div className="relative z-10">
          <div className="flex items-center justify-between mb-4">
            <div>
              <p className="text-gray-400 text-sm font-bold tracking-wide uppercase">Total Balance</p>
              <h2 className="text-4xl font-bold text-white mt-2">
                ${totalBalance.toLocaleString('en-US', { minimumFractionDigits: 2 })}
              </h2>
            </div>
            <button
              onClick={refreshBalances}
              disabled={isLoading}
              className="p-3 bg-[#1a1a2e] hover:bg-[#2a2a3e] rounded-xl transition-colors disabled:opacity-50"
            >
              {isLoading ? (
                <Loader2 className="w-5 h-5 text-blue-400 animate-spin" />
              ) : (
                <RefreshIcon />
              )}
            </button>
          </div>

          {/* Token Balances */}
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4 mt-6">
            {tokens.map((token) => (
              <div key={token.symbol} className="bg-[#0a0a0f] border border-[#2a2a35] rounded-xl p-4 flex items-center justify-between">
                <div className="flex items-center gap-3">
                  <div className={`w-10 h-10 rounded-full flex items-center justify-center text-lg ${token.color || 'bg-gray-700'}`}>
                    {token.icon}
                  </div>
                  <div>
                    <div className="text-white font-semibold">{token.name}</div>
                    <div className="text-xs text-gray-400">{token.symbol}</div>
                  </div>
                </div>
                <div className="text-right">
                  <div className="text-white font-bold">{token.balance.toLocaleString()}</div>
                  <div className="text-xs text-gray-400">
                    ${token.value.toLocaleString('en-US', { minimumFractionDigits: 2 })}
                  </div>
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* Quick Actions */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
        <QuickAction icon={<Send className="w-5 h-5" />} label="Send" color="from-blue-500 to-cyan-500" />
        <QuickAction icon={<ArrowDownLeft className="w-5 h-5" />} label="Receive" color="from-green-500 to-emerald-500" />
        <QuickAction icon={<ArrowLeftRight className="w-5 h-5" />} label="Swap" color="from-purple-500 to-pink-500" />
        <QuickAction icon={<History className="w-5 h-5" />} label="History" color="from-orange-500 to-red-500" />
      </div>

      {/* Recent Transactions */}
      <div className="bg-[#111] border border-[#2a2a35] rounded-2xl overflow-hidden">
        <div className="p-4 border-b border-[#2a2a35] flex items-center justify-between">
          <h3 className="text-lg font-semibold text-white">Recent Transactions</h3>
          <button className="text-sm text-blue-400 hover:text-blue-300">View All</button>
        </div>
        <div className="divide-y divide-[#2a2a35]">
          {transactions.slice(0, 5).map((tx) => (
            <TransactionItem key={tx.id} transaction={tx} />
          ))}
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// Transaction History Panel
// ============================================================================

export function WalletTransactionsPanel({ className }: WalletPanelProps) {
  const { transactions = [] } = useWalletStore();
  const [filter, setFilter] = useState<'all' | 'send' | 'receive' | 'swap'>('all');
  const [search, setSearch] = useState('');

  const filteredTransactions = transactions.filter((tx) => {
    if (filter !== 'all' && tx.type !== filter) return false;
    if (search && !tx.hash.toLowerCase().includes(search.toLowerCase())) return false;
    return true;
  });

  return (
    <div className={`space-y-6 ${className}`}>
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold text-white">Transaction History</h2>
          <p className="text-sm text-gray-400">View and manage your transaction history</p>
        </div>
        <div className="flex items-center gap-2">
          <div className="relative">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400" />
            <input
              type="text"
              placeholder="Search transactions..."
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="pl-9 pr-4 py-2 bg-[#1a1a2e] border border-[#2a2a35] rounded-lg text-sm text-white focus:outline-none focus:border-blue-500"
            />
          </div>
          <select
            value={filter}
            onChange={(e) => setFilter(e.target.value as any)}
            className="px-4 py-2 bg-[#1a1a2e] border border-[#2a2a35] rounded-lg text-sm text-white focus:outline-none focus:border-blue-500"
          >
            <option value="all">All Types</option>
            <option value="send">Send</option>
            <option value="receive">Receive</option>
            <option value="swap">Swap</option>
          </select>
        </div>
      </div>

      <div className="bg-[#111] border border-[#2a2a35] rounded-2xl overflow-hidden">
        <div className="overflow-x-auto">
          <table className="w-full">
            <thead className="bg-[#0a0a0f]">
              <tr>
                <th className="px-6 py-4 text-left text-xs font-medium text-gray-400 uppercase tracking-wider">Type</th>
                <th className="px-6 py-4 text-left text-xs font-medium text-gray-400 uppercase tracking-wider">Status</th>
                <th className="px-6 py-4 text-left text-xs font-medium text-gray-400 uppercase tracking-wider">Amount</th>
                <th className="px-6 py-4 text-left text-xs font-medium text-gray-400 uppercase tracking-wider">Hash</th>
                <th className="px-6 py-4 text-left text-xs font-medium text-gray-400 uppercase tracking-wider">Time</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-[#2a2a35]">
              {filteredTransactions.map((tx) => (
                <tr key={tx.id} className="hover:bg-[#1a1a2e] transition-colors">
                  <td className="px-6 py-4">
                    <TransactionTypeBadge type={tx.type} />
                  </td>
                  <td className="px-6 py-4">
                    <TransactionStatusBadge status={tx.status} />
                  </td>
                  <td className="px-6 py-4">
                    <div className="flex items-center gap-2">
                      <span className="text-white font-medium">{tx.amount} {tx.symbol}</span>
                      <span className="text-xs text-gray-400">{tx.network.toUpperCase()}</span>
                    </div>
                  </td>
                  <td className="px-6 py-4">
                    <div className="flex items-center gap-2">
                      <span className="text-xs text-gray-400 font-mono">{tx.hash.slice(0, 16)}...</span>
                      <button className="text-blue-400 hover:text-blue-300">
                        <Copy className="w-3 h-3" />
                      </button>
                    </div>
                  </td>
                  <td className="px-6 py-4">
                    <div className="text-sm text-gray-400">
                      {new Date(tx.timestamp).toLocaleString()}
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        {filteredTransactions.length === 0 && (
          <div className="p-8 text-center text-gray-400">
            No transactions found
          </div>
        )}
      </div>
    </div>
  );
}

// ============================================================================
// Wallet Settings Panel
// ============================================================================

export function WalletSettingsPanel({ className }: WalletPanelProps) {
  const { universalWallet, disconnect } = useWalletStore();
  const [showMnemonic, setShowMnemonic] = useState(false);

  return (
    <div className={`space-y-6 ${className}`}>
      <div>
        <h2 className="text-2xl font-bold text-white">Wallet Settings</h2>
        <p className="text-sm text-gray-400">Configure your wallet preferences and security</p>
      </div>

      {/* Wallet Info */}
      <div className="bg-[#111] border border-[#2a2a35] rounded-2xl p-6">
        <h3 className="text-lg font-semibold text-white mb-4">Wallet Information</h3>
        <div className="space-y-4">
          <div>
            <label className="text-xs text-gray-400 uppercase tracking-wider">Wallet ID</label>
            <div className="mt-1 flex items-center gap-2">
              <code className="flex-1 bg-[#0a0a0f] border border-[#2a2a35] rounded-lg px-3 py-2 text-sm text-gray-300 font-mono">
                {universalWallet?.evm_address || 'Not connected'}
              </code>
              <button className="p-2 bg-[#1a1a2e] hover:bg-[#2a2a3e] rounded-lg transition-colors">
                <Copy className="w-4 h-4 text-gray-400" />
              </button>
            </div>
          </div>
          <div>
            <label className="text-xs text-gray-400 uppercase tracking-wider">Network</label>
            <div className="mt-1">
              <select className="w-full bg-[#0a0a0f] border border-[#2a2a35] rounded-lg px-3 py-2 text-sm text-white focus:outline-none focus:border-blue-500">
                <option value="mainnet">X3 Mainnet</option>
                <option value="testnet">X3 Testnet</option>
                <option value="local">Local Devnet</option>
              </select>
            </div>
          </div>
        </div>
      </div>

      {/* Security Settings */}
      <div className="bg-[#111] border border-[#2a2a35] rounded-2xl p-6">
        <h3 className="text-lg font-semibold text-white mb-4">Security Settings</h3>
        <div className="space-y-4">
          <SecuritySetting
            icon={<LockKeyhole className="w-5 h-5" />}
            title="PIN Protection"
            description="Require PIN to access wallet"
            enabled={true}
          />
          <SecuritySetting
            icon={<Fingerprint className="w-5 h-5" />}
            title="Biometric Auth"
            description="Use fingerprint/face ID for quick access"
            enabled={false}
          />
          <SecuritySetting
            icon={<ShieldCheck className="w-5 h-5" />}
            title="Hardware Wallet"
            description="Connect Ledger or Trezor for enhanced security"
            enabled={false}
          />
        </div>
      </div>

      {/* Backup & Recovery */}
      <div className="bg-[#111] border border-[#2a2a35] rounded-2xl p-6">
        <h3 className="text-lg font-semibold text-white mb-4">Backup & Recovery</h3>
        <div className="space-y-4">
          <div>
            <label className="text-xs text-gray-400 uppercase tracking-wider">Recovery Phrase</label>
            <div className="mt-1">
              <div className="bg-[#0a0a0f] border border-[#2a2a35] rounded-lg p-4">
                <div className="flex items-center justify-between mb-2">
                  <span className="text-sm text-gray-300">
                    {showMnemonic ? universalWallet?.mnemonic : '••••••••••••••••'}
                  </span>
                  <button
                    onClick={() => setShowMnemonic(!showMnemonic)}
                    className="text-blue-400 hover:text-blue-300 text-sm"
                  >
                    {showMnemonic ? 'Hide' : 'Show'}
                  </button>
                </div>
                <p className="text-xs text-yellow-500/80">
                  ⚠️ Never share your recovery phrase with anyone
                </p>
              </div>
            </div>
          </div>
          <div className="flex gap-3">
            <button className="flex-1 flex items-center justify-center gap-2 px-4 py-2 bg-[#1a1a2e] hover:bg-[#2a2a3e] rounded-lg text-sm text-white transition-colors">
              <Download className="w-4 h-4" />
              Export Backup
            </button>
            <button className="flex-1 flex items-center justify-center gap-2 px-4 py-2 bg-[#1a1a2e] hover:bg-[#2a2a3e] rounded-lg text-sm text-white transition-colors">
              <QrCode className="w-4 h-4" />
              QR Code
            </button>
          </div>
        </div>
      </div>

      {/* Disconnect Button */}
      <div className="flex justify-end">
        <button
          onClick={disconnect}
          className="flex items-center gap-2 px-6 py-3 bg-red-500/20 hover:bg-red-500/30 text-red-400 rounded-xl transition-colors"
        >
          <LogOutIcon />
          <span className="font-medium">Disconnect Wallet</span>
        </button>
      </div>
    </div>
  );
}

// ============================================================================
// Helper Components
// ============================================================================

function QuickAction({ icon, label, color }: { icon: React.ReactNode; label: string; color: string }) {
  return (
    <button className={`flex flex-col items-center justify-center gap-2 p-4 bg-gradient-to-br ${color} rounded-2xl shadow-lg transition-all hover:scale-105 hover:shadow-xl`}>
      <div className="p-2 bg-white/20 rounded-lg">{icon}</div>
      <span className="text-sm font-medium text-white">{label}</span>
    </button>
  );
}

function TransactionItem({ transaction }: { transaction: Transaction }) {
  return (
    <div className="p-4 hover:bg-[#1a1a2e] transition-colors">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className={`p-2 rounded-lg ${
            transaction.type === 'receive' ? 'bg-green-500/20 text-green-400' :
            transaction.type === 'send' ? 'bg-red-500/20 text-red-400' :
            'bg-blue-500/20 text-blue-400'
          }`}>
            {transaction.type === 'receive' ? <ArrowDownLeft className="w-4 h-4" /> :
             transaction.type === 'send' ? <Send className="w-4 h-4" /> :
             <ArrowLeftRight className="w-4 h-4" />}
          </div>
          <div>
            <div className="text-white font-medium capitalize">{transaction.type}</div>
            <div className="text-xs text-gray-400">
              {transaction.type === 'receive' ? 'From' : 'To'}: {formatAddress(transaction.from)}
            </div>
          </div>
        </div>
        <div className="text-right">
          <div className={`font-bold ${
            transaction.type === 'receive' ? 'text-green-400' :
            transaction.type === 'send' ? 'text-red-400' :
            'text-white'
          }`}>
            {transaction.type === 'receive' ? '+' : '-'}{transaction.amount} {transaction.symbol}
          </div>
          <div className="text-xs text-gray-400">
            {new Date(transaction.timestamp).toLocaleString()}
          </div>
        </div>
      </div>
    </div>
  );
}

function TransactionTypeBadge({ type }: { type: Transaction['type'] }) {
  const colors = {
    send: 'bg-red-500/20 text-red-400',
    receive: 'bg-green-500/20 text-green-400',
    swap: 'bg-blue-500/20 text-blue-400',
    comit: 'bg-purple-500/20 text-purple-400',
    mint: 'bg-yellow-500/20 text-yellow-400',
  };

  return (
    <span className={`px-2 py-1 rounded text-xs font-medium capitalize ${colors[type]}`}>
      {type}
    </span>
  );
}

function TransactionStatusBadge({ status }: { status: Transaction['status'] }) {
  const colors = {
    confirmed: 'bg-green-500/20 text-green-400',
    pending: 'bg-yellow-500/20 text-yellow-400',
    failed: 'bg-red-500/20 text-red-400',
  };

  return (
    <span className={`px-2 py-1 rounded text-xs font-medium capitalize ${colors[status]}`}>
      {status}
    </span>
  );
}

function SecuritySetting({ icon, title, description, enabled }: { icon: React.ReactNode; title: string; description: string; enabled: boolean }) {
  return (
    <div className="flex items-center justify-between p-3 bg-[#0a0a0f] border border-[#2a2a35] rounded-lg">
      <div className="flex items-center gap-3">
        <div className={`p-2 rounded-lg ${enabled ? 'bg-blue-500/20 text-blue-400' : 'bg-gray-700/50 text-gray-400'}`}>
          {icon}
        </div>
        <div>
          <div className="text-sm font-medium text-white">{title}</div>
          <div className="text-xs text-gray-400">{description}</div>
        </div>
      </div>
      <div className={`w-10 h-5 rounded-full relative transition-colors ${enabled ? 'bg-blue-500' : 'bg-gray-700'}`}>
        <div className={`absolute top-1 w-3 h-3 rounded-full bg-white transition-all ${enabled ? 'left-6' : 'left-1'}`} />
      </div>
    </div>
  );
}

// ============================================================================
// Icons
// ============================================================================

function RefreshIcon() {
  return <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
  </svg>;
}

function LogOutIcon() {
  return <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1" />
  </svg>;
}
