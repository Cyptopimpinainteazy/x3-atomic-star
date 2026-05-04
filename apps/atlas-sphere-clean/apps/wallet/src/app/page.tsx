'use client';

import { useState, useEffect } from 'react';
import Link from 'next/link';
import { 
  LayoutDashboard, Send, Receive, ArrowLeftRight, History, Settings, 
  Shield, Lock, Key, Globe, ChevronDown, Plus, Search, Filter,
  Clock, CheckCircle, AlertCircle, Loader2, Copy, QrCode, Download,
  Trash2, Eye, EyeOff, Smartphone, Fingerprint, ShieldCheck, LockKeyhole,
  Wallet, CreditCard, BarChart3, Users, Zap, TrendingUp
} from 'lucide-react';

// ============================================================================
// Types
// ============================================================================

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
}

interface Network {
  id: string;
  name: string;
  chainId: number;
  rpc: string;
  isTestnet: boolean;
}

// ============================================================================
// Mock Data
// ============================================================================

const NETWORKS: Network[] = [
  { id: 'mainnet', name: 'X3 Mainnet', chainId: 123456789, rpc: 'https://rpc.x3chain.io', isTestnet: false },
  { id: 'testnet', name: 'X3 Testnet', chainId: 123456788, rpc: 'https://rpc-testnet.x3chain.io', isTestnet: true },
  { id: 'local', name: 'Local Devnet', chainId: 123456787, rpc: 'http://localhost:9933', isTestnet: true },
];

const MOCK_TOKENS: Token[] = [
  { symbol: 'X3', name: 'X3 Sphere', balance: 1250.0, value: 3750.0, change24h: 5.2, icon: '⭐', network: 'substrate', color: 'from-orange-500 to-yellow-500' },
  { symbol: 'ETH', name: 'Ethereum', balance: 2.45, value: 8304.50, change24h: -1.3, icon: '◆', network: 'evm', color: 'from-blue-500 to-indigo-500' },
  { symbol: 'SOL', name: 'Solana', balance: 15.8, value: 1580.0, change24h: 3.8, icon: '◎', network: 'svm', color: 'from-purple-500 to-pink-500' },
  { symbol: 'USDC', name: 'USD Coin', balance: 500.0, value: 500.0, change24h: 0.01, icon: '$', network: 'evm', color: 'from-green-500 to-emerald-500' },
];

const MOCK_TRANSACTIONS: Transaction[] = [
  { id: '1', type: 'receive', amount: 500, symbol: 'USDC', timestamp: Date.now() - 600000, status: 'confirmed', from: '0x...', to: '0x...', hash: '0x...', network: 'evm' },
  { id: '2', type: 'swap', amount: 2.5, symbol: 'ETH', timestamp: Date.now() - 7200000, status: 'confirmed', from: '0x...', to: '0x...', hash: '0x...', network: 'evm' },
  { id: '3', type: 'send', amount: 125, symbol: 'X3', timestamp: Date.now() - 86400000, status: 'pending', from: '0x...', to: '0x...', hash: '0x...', network: 'substrate' },
];

// ============================================================================
// Components
// ============================================================================

export default function WalletApp() {
  const [activeTab, setActiveTab] = useState<'dashboard' | 'send' | 'receive' | 'swap' | 'history' | 'settings'>('dashboard');
  const [selectedNetwork, setSelectedNetwork] = useState<Network>(NETWORKS[0]);
  const [isNetworkOpen, setIsNetworkOpen] = useState(false);
  const [isLoading, setIsLoading] = useState(false);

  const totalBalance = MOCK_TOKENS.reduce((sum, t) => sum + t.value, 0);

  const refreshBalances = () => {
    setIsLoading(true);
    setTimeout(() => setIsLoading(false), 1000);
  };

  return (
    <div className="min-h-screen bg-gradient-to-b from-[#0a0a0f] via-[#111116] to-[#0a0a0f]">
      {/* Navigation Header */}
      <nav className="border-b border-[#2a2a35] bg-[#111116]/50 backdrop-blur sticky top-0 z-50">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 bg-gradient-to-br from-blue-500 to-purple-600 rounded-xl flex items-center justify-center">
                <Wallet className="w-6 h-6 text-white" />
              </div>
              <div>
                <h1 className="text-2xl font-bold text-white">X3 Wallet</h1>
                <p className="text-xs text-gray-400">Multi-Chain Web Wallet</p>
              </div>
            </div>

            {/* Network Selector */}
            <div className="relative">
              <button
                onClick={() => setIsNetworkOpen(!isNetworkOpen)}
                className="flex items-center gap-2 px-4 py-2 bg-[#1a1a2e] border border-[#2a2a35] rounded-lg hover:border-[#3a3a45] transition-colors"
              >
                <Globe className={`w-4 h-4 ${selectedNetwork.isTestnet ? 'text-yellow-400' : 'text-green-400'}`} />
                <span className="text-sm font-medium text-white">{selectedNetwork.name}</span>
                <ChevronDown className={`w-4 h-4 text-gray-400 transition-transform ${isNetworkOpen ? 'rotate-180' : ''}`} />
              </button>

              {isNetworkOpen && (
                <>
                  <div className="fixed inset-0 z-10" onClick={() => setIsNetworkOpen(false)} />
                  <div className="absolute right-0 top-full mt-2 w-64 z-20 bg-[#0a0a0f] border border-[#2a2a35] rounded-xl shadow-2xl overflow-hidden">
                    <div className="p-2">
                      {NETWORKS.map((net) => (
                        <button
                          key={net.id}
                          onClick={() => {
                            setSelectedNetwork(net);
                            setIsNetworkOpen(false);
                          }}
                          className={`w-full flex items-center gap-3 px-3 py-2.5 rounded-lg transition-colors ${
                            selectedNetwork.id === net.id
                              ? 'bg-blue-500/20 text-blue-400'
                              : 'hover:bg-[#1a1a2e] text-gray-300'
                          }`}
                        >
                          <Globe className={`w-4 h-4 ${net.isTestnet ? 'text-yellow-400' : 'text-green-400'}`} />
                          <div className="flex-1 text-left">
                            <div className="text-sm font-medium">{net.name}</div>
                            <div className="text-xs opacity-60">{net.rpc}</div>
                          </div>
                          {selectedNetwork.id === net.id && <CheckCircle className="w-4 h-4" />}
                        </button>
                      ))}
                    </div>
                  </div>
                </>
              )}
            </div>
          </div>
        </div>
      </nav>

      {/* Main Content */}
      <main className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
        {/* Dashboard Tab */}
        {activeTab === 'dashboard' && (
          <div className="space-y-6 animate-in fade-in duration-500">
            {/* Balance Card */}
            <div className="bg-gradient-to-br from-[#111] via-[#151515] to-[#0a0a0f] border border-[#222] p-8 rounded-3xl shadow-2xl relative overflow-hidden">
              <div className="absolute top-0 right-0 p-12 opacity-20 pointer-events-none">
                <Globe className="w-64 h-64 text-blue-500/20 animate-spin-slow" />
              </div>
              
              <div className="relative z-10">
                <div className="flex items-center justify-between mb-6">
                  <div>
                    <p className="text-gray-400 text-sm font-bold tracking-wide uppercase">Total Balance</p>
                    <h2 className="text-5xl font-bold text-white mt-2">
                      ${totalBalance.toLocaleString('en-US', { minimumFractionDigits: 2 })}
                    </h2>
                  </div>
                  <button
                    onClick={refreshBalances}
                    disabled={isLoading}
                    className="p-4 bg-[#1a1a2e] hover:bg-[#2a2a3e] rounded-xl transition-colors disabled:opacity-50"
                  >
                    {isLoading ? (
                      <Loader2 className="w-6 h-6 text-blue-400 animate-spin" />
                    ) : (
                      <svg className="w-6 h-6 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                      </svg>
                    )}
                  </button>
                </div>

                {/* Token Balances */}
                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4 mt-8">
                  {MOCK_TOKENS.map((token) => (
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
              <QuickAction 
                icon={<Send className="w-5 h-5" />} 
                label="Send" 
                color="from-blue-500 to-cyan-500"
                onClick={() => setActiveTab('send')}
              />
              <QuickAction 
                icon={<Receive className="w-5 h-5" />} 
                label="Receive" 
                color="from-green-500 to-emerald-500"
                onClick={() => setActiveTab('receive')}
              />
              <QuickAction 
                icon={<ArrowLeftRight className="w-5 h-5" />} 
                label="Swap" 
                color="from-purple-500 to-pink-500"
                onClick={() => setActiveTab('swap')}
              />
              <QuickAction 
                icon={<History className="w-5 h-5" />} 
                label="History" 
                color="from-orange-500 to-red-500"
                onClick={() => setActiveTab('history')}
              />
            </div>

            {/* Recent Transactions */}
            <div className="bg-[#111] border border-[#2a2a35] rounded-2xl overflow-hidden">
              <div className="p-4 border-b border-[#2a2a35] flex items-center justify-between">
                <h3 className="text-lg font-semibold text-white">Recent Transactions</h3>
                <button 
                  onClick={() => setActiveTab('history')}
                  className="text-sm text-blue-400 hover:text-blue-300"
                >
                  View All
                </button>
              </div>
              <div className="divide-y divide-[#2a2a35]">
                {MOCK_TRANSACTIONS.slice(0, 5).map((tx) => (
                  <TransactionItem key={tx.id} transaction={tx} />
                ))}
              </div>
            </div>

            {/* Network Stats */}
            <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
              <StatCard 
                icon={<Users className="w-5 h-5" />} 
                label="Active Nodes" 
                value="1,234" 
                trend="+12%" 
                trendUp={true}
              />
              <StatCard 
                icon={<Zap className="w-5 h-5" />} 
                label="Network Speed" 
                value="450 ms" 
                trend="-5%" 
                trendUp={true}
              />
              <StatCard 
                icon={<BarChart3 className="w-5 h-5" />} 
                label="Gas Price" 
                value="25 Gwei" 
                trend="+2%" 
                trendUp={false}
              />
            </div>
          </div>
        )}

        {/* Send Tab */}
        {activeTab === 'send' && (
          <SendView 
            tokens={MOCK_TOKENS} 
            network={selectedNetwork}
            onBack={() => setActiveTab('dashboard')}
          />
        )}

        {/* Receive Tab */}
        {activeTab === 'receive' && (
          <ReceiveView 
            tokens={MOCK_TOKENS} 
            network={selectedNetwork}
            onBack={() => setActiveTab('dashboard')}
          />
        )}

        {/* Swap Tab */}
        {activeTab === 'swap' && (
          <SwapView 
            tokens={MOCK_TOKENS} 
            network={selectedNetwork}
            onBack={() => setActiveTab('dashboard')}
          />
        )}

        {/* History Tab */}
        {activeTab === 'history' && (
          <HistoryView 
            transactions={MOCK_TRANSACTIONS}
            onBack={() => setActiveTab('dashboard')}
          />
        )}

        {/* Settings Tab */}
        {activeTab === 'settings' && (
          <SettingsView 
            network={selectedNetwork}
            onBack={() => setActiveTab('dashboard')}
          />
        )}
      </main>

      {/* Bottom Navigation */}
      <div className="fixed bottom-0 left-0 right-0 bg-[#111116] border-t border-[#2a2a35] p-4 z-50">
        <div className="max-w-7xl mx-auto flex justify-around">
          <NavButton 
            icon={<LayoutDashboard className="w-6 h-6" />} 
            label="Dashboard" 
            active={activeTab === 'dashboard'}
            onClick={() => setActiveTab('dashboard')}
          />
          <NavButton 
            icon={<Send className="w-6 h-6" />} 
            label="Send" 
            active={activeTab === 'send'}
            onClick={() => setActiveTab('send')}
          />
          <NavButton 
            icon={<History className="w-6 h-6" />} 
            label="History" 
            active={activeTab === 'history'}
            onClick={() => setActiveTab('history')}
          />
          <NavButton 
            icon={<Settings className="w-6 h-6" />} 
            label="Settings" 
            active={activeTab === 'settings'}
            onClick={() => setActiveTab('settings')}
          />
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// Sub-Views
// ============================================================================

function SendView({ tokens, network, onBack }: { tokens: Token[]; network: Network; onBack: () => void }) {
  const [selectedToken, setSelectedToken] = useState(tokens[0]);
  const [amount, setAmount] = useState('');
  const [toAddress, setToAddress] = useState('');
  const [isSending, setIsSending] = useState(false);

  const handleSend = () => {
    setIsSending(true);
    setTimeout(() => {
      setIsSending(false);
      onBack();
    }, 2000);
  };

  return (
    <div className="space-y-6 animate-in slide-in-from-right duration-500">
      <div className="flex items-center gap-4 mb-6">
        <button 
          onClick={onBack}
          className="p-2 bg-[#1a1a2e] hover:bg-[#2a2a3e] rounded-lg transition-colors"
        >
          <svg className="w-6 h-6 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
          </svg>
        </button>
        <h2 className="text-2xl font-bold text-white">Send Assets</h2>
      </div>

      <div className="bg-[#111] border border-[#2a2a35] rounded-2xl p-6 space-y-6">
        {/* Token Selection */}
        <div>
          <label className="text-xs text-gray-400 uppercase tracking-wider mb-2 block">Select Token</label>
          <div className="relative">
            <select
              value={selectedToken.symbol}
              onChange={(e) => setSelectedToken(tokens.find(t => t.symbol === e.target.value) || tokens[0])}
              className="w-full bg-[#0a0a0f] border border-[#2a2a35] rounded-xl px-4 py-3 text-white focus:outline-none focus:border-blue-500 appearance-none"
            >
              {tokens.map((token) => (
                <option key={token.symbol} value={token.symbol}>
                  {token.name} ({token.symbol})
                </option>
              ))}
            </select>
            <div className="absolute right-4 top-1/2 -translate-y-1/2 pointer-events-none">
              <ChevronDown className="w-5 h-5 text-gray-400" />
            </div>
          </div>
        </div>

        {/* Amount Input */}
        <div>
          <label className="text-xs text-gray-400 uppercase tracking-wider mb-2 block">Amount</label>
          <div className="relative">
            <input
              type="number"
              value={amount}
              onChange={(e) => setAmount(e.target.value)}
              placeholder="0.00"
              className="w-full bg-[#0a0a0f] border border-[#2a2a35] rounded-xl px-4 py-3 text-white focus:outline-none focus:border-blue-500"
            />
            <div className="absolute right-4 top-1/2 -translate-y-1/2 text-gray-400 font-medium">
              {selectedToken.symbol}
            </div>
          </div>
          <div className="mt-2 text-xs text-gray-500">
            Available: {selectedToken.balance.toLocaleString()} {selectedToken.symbol}
          </div>
        </div>

        {/* To Address */}
        <div>
          <label className="text-xs text-gray-400 uppercase tracking-wider mb-2 block">To Address</label>
          <input
            type="text"
            value={toAddress}
            onChange={(e) => setToAddress(e.target.value)}
            placeholder="0x..."
            className="w-full bg-[#0a0a0f] border border-[#2a2a35] rounded-xl px-4 py-3 text-white focus:outline-none focus:border-blue-500"
          />
        </div>

        {/* Network Info */}
        <div className="flex items-center justify-between p-3 bg-[#0a0a0f] rounded-lg">
          <div className="flex items-center gap-2">
            <Globe className="w-4 h-4 text-blue-400" />
            <span className="text-sm text-gray-300">{network.name}</span>
          </div>
          <span className="text-xs text-gray-500">Estimated fee: $0.50</span>
        </div>

        {/* Send Button */}
        <button
          onClick={handleSend}
          disabled={!amount || !toAddress || isSending}
          className="w-full py-4 bg-gradient-to-r from-blue-500 to-purple-600 hover:from-blue-600 hover:to-purple-700 text-white font-bold rounded-xl transition-all disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
        >
          {isSending ? (
            <>
              <Loader2 className="w-5 h-5 animate-spin" />
              Sending...
            </>
          ) : (
            'Send Assets'
          )}
        </button>
      </div>
    </div>
  );
}

function ReceiveView({ tokens, network, onBack }: { tokens: Token[]; network: Network; onBack: () => void }) {
  const [selectedToken, setSelectedToken] = useState(tokens[0]);
  const [showAddress, setShowAddress] = useState(false);

  return (
    <div className="space-y-6 animate-in slide-in-from-right duration-500">
      <div className="flex items-center gap-4 mb-6">
        <button 
          onClick={onBack}
          className="p-2 bg-[#1a1a2e] hover:bg-[#2a2a3e] rounded-lg transition-colors"
        >
          <svg className="w-6 h-6 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
          </svg>
        </button>
        <h2 className="text-2xl font-bold text-white">Receive Assets</h2>
      </div>

      <div className="bg-[#111] border border-[#2a2a35] rounded-2xl p-6 space-y-6">
        {/* Token Selection */}
        <div>
          <label className="text-xs text-gray-400 uppercase tracking-wider mb-2 block">Select Token</label>
          <div className="relative">
            <select
              value={selectedToken.symbol}
              onChange={(e) => setSelectedToken(tokens.find(t => t.symbol === e.target.value) || tokens[0])}
              className="w-full bg-[#0a0a0f] border border-[#2a2a35] rounded-xl px-4 py-3 text-white focus:outline-none focus:border-blue-500 appearance-none"
            >
              {tokens.map((token) => (
                <option key={token.symbol} value={token.symbol}>
                  {token.name} ({token.symbol})
                </option>
              ))}
            </select>
            <div className="absolute right-4 top-1/2 -translate-y-1/2 pointer-events-none">
              <ChevronDown className="w-5 h-5 text-gray-400" />
            </div>
          </div>
        </div>

        {/* QR Code */}
        <div className="flex flex-col items-center justify-center p-6 bg-white rounded-xl">
          <div className="w-48 h-48 bg-white p-2">
            {/* QR Code placeholder */}
            <div className="w-full h-full bg-gradient-to-br from-blue-500 to-purple-600 rounded-lg flex items-center justify-center">
              <Wallet className="w-16 h-16 text-white" />
            </div>
          </div>
          <p className="mt-4 text-sm text-gray-400">Scan to receive {selectedToken.symbol}</p>
        </div>

        {/* Address */}
        <div>
          <label className="text-xs text-gray-400 uppercase tracking-wider mb-2 block">Your Address</label>
          <div className="relative">
            <input
              type="text"
              readOnly
              value={showAddress ? '0x1234567890abcdef1234567890abcdef12345678' : '••••••••••••••••'}
              className="w-full bg-[#0a0a0f] border border-[#2a2a35] rounded-xl px-4 py-3 text-white focus:outline-none focus:border-blue-500 font-mono"
            />
            <div className="absolute right-2 top-1/2 -translate-y-1/2 flex gap-2">
              <button
                onClick={() => setShowAddress(!showAddress)}
                className="p-2 bg-[#1a1a2e] hover:bg-[#2a2a3e] rounded-lg transition-colors"
              >
                {showAddress ? <EyeOff className="w-4 h-4 text-gray-400" /> : <Eye className="w-4 h-4 text-gray-400" />}
              </button>
              <button className="p-2 bg-[#1a1a2e] hover:bg-[#2a2a3e] rounded-lg transition-colors">
                <Copy className="w-4 h-4 text-gray-400" />
              </button>
            </div>
          </div>
        </div>

        {/* Network Info */}
        <div className="flex items-center justify-between p-3 bg-[#0a0a0f] rounded-lg">
          <div className="flex items-center gap-2">
            <Globe className="w-4 h-4 text-blue-400" />
            <span className="text-sm text-gray-300">{network.name}</span>
          </div>
          <span className="text-xs text-gray-500">Network: {selectedToken.network.toUpperCase()}</span>
        </div>
      </div>
    </div>
  );
}

function SwapView({ tokens, network, onBack }: { tokens: Token[]; network: Network; onBack: () => void }) {
  const [fromToken, setFromToken] = useState(tokens[0]);
  const [toToken, setToToken] = useState(tokens[1]);
  const [amount, setAmount] = useState('');
  const [isSwapping, setIsSwapping] = useState(false);

  const handleSwap = () => {
    setIsSwapping(true);
    setTimeout(() => {
      setIsSwapping(false);
      onBack();
    }, 2000);
  };

  return (
    <div className="space-y-6 animate-in slide-in-from-right duration-500">
      <div className="flex items-center gap-4 mb-6">
        <button 
          onClick={onBack}
          className="p-2 bg-[#1a1a2e] hover:bg-[#2a2a3e] rounded-lg transition-colors"
        >
          <svg className="w-6 h-6 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
          </svg>
        </button>
        <h2 className="text-2xl font-bold text-white">Swap Assets</h2>
      </div>

      <div className="bg-[#111] border border-[#2a2a35] rounded-2xl p-6 space-y-6">
        {/* From */}
        <div>
          <label className="text-xs text-gray-400 uppercase tracking-wider mb-2 block">You Pay</label>
          <div className="relative">
            <input
              type="number"
              value={amount}
              onChange={(e) => setAmount(e.target.value)}
              placeholder="0.00"
              className="w-full bg-[#0a0a0f] border border-[#2a2a35] rounded-xl px-4 py-3 text-white focus:outline-none focus:border-blue-500"
            />
            <div className="absolute right-4 top-1/2 -translate-y-1/2 flex items-center gap-2">
              <select
                value={fromToken.symbol}
                onChange={(e) => setFromToken(tokens.find(t => t.symbol === e.target.value) || tokens[0])}
                className="bg-[#1a1a2e] border border-[#2a2a35] rounded-lg px-3 py-1 text-sm text-white focus:outline-none"
              >
                {tokens.map((token) => (
                  <option key={token.symbol} value={token.symbol}>
                    {token.symbol}
                  </option>
                ))}
              </select>
            </div>
          </div>
        </div>

        {/* Swap Arrow */}
        <div className="flex justify-center">
          <button 
            onClick={() => {
              const temp = fromToken;
              setFromToken(toToken);
              setToToken(temp);
            }}
            className="p-2 bg-[#1a1a2e] hover:bg-[#2a2a3e] rounded-full transition-colors"
          >
            <ArrowLeftRight className="w-5 h-5 text-gray-400" />
          </button>
        </div>

        {/* To */}
        <div>
          <label className="text-xs text-gray-400 uppercase tracking-wider mb-2 block">You Receive</label>
          <div className="relative">
            <input
              type="text"
              readOnly
              value={amount ? (parseFloat(amount) * 0.95).toFixed(4) : ''}
              className="w-full bg-[#0a0a0f] border border-[#2a2a35] rounded-xl px-4 py-3 text-white focus:outline-none focus:border-blue-500"
            />
            <div className="absolute right-4 top-1/2 -translate-y-1/2 flex items-center gap-2">
              <select
                value={toToken.symbol}
                onChange={(e) => setToToken(tokens.find(t => t.symbol === e.target.value) || tokens[1])}
                className="bg-[#1a1a2e] border border-[#2a2a35] rounded-lg px-3 py-1 text-sm text-white focus:outline-none"
              >
                {tokens.map((token) => (
                  <option key={token.symbol} value={token.symbol}>
                    {token.symbol}
                  </option>
                ))}
              </select>
            </div>
          </div>
        </div>

        {/* Rate Info */}
        <div className="flex items-center justify-between p-3 bg-[#0a0a0f] rounded-lg">
          <span className="text-sm text-gray-400">Rate</span>
          <span className="text-sm text-white">1 {fromToken.symbol} ≈ {0.95} {toToken.symbol}</span>
        </div>

        {/* Network Info */}
        <div className="flex items-center justify-between p-3 bg-[#0a0a0f] rounded-lg">
          <div className="flex items-center gap-2">
            <Globe className="w-4 h-4 text-blue-400" />
            <span className="text-sm text-gray-300">{network.name}</span>
          </div>
          <span className="text-xs text-gray-500">Fee: $0.50</span>
        </div>

        {/* Swap Button */}
        <button
          onClick={handleSwap}
          disabled={!amount || isSwapping}
          className="w-full py-4 bg-gradient-to-r from-purple-500 to-pink-600 hover:from-purple-600 hover:to-pink-700 text-white font-bold rounded-xl transition-all disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
        >
          {isSwapping ? (
            <>
              <Loader2 className="w-5 h-5 animate-spin" />
              Swapping...
            </>
          ) : (
            'Swap'
          )}
        </button>
      </div>
    </div>
  );
}

function HistoryView({ transactions, onBack }: { transactions: Transaction[]; onBack: () => void }) {
  const [filter, setFilter] = useState<'all' | 'send' | 'receive' | 'swap'>('all');

  const filteredTransactions = transactions.filter(tx => filter === 'all' || tx.type === filter);

  return (
    <div className="space-y-6 animate-in slide-in-from-right duration-500">
      <div className="flex items-center gap-4 mb-6">
        <button 
          onClick={onBack}
          className="p-2 bg-[#1a1a2e] hover:bg-[#2a2a3e] rounded-lg transition-colors"
        >
          <svg className="w-6 h-6 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
          </svg>
        </button>
        <h2 className="text-2xl font-bold text-white">Transaction History</h2>
      </div>

      {/* Filters */}
      <div className="flex gap-2 overflow-x-auto pb-2">
        {(['all', 'send', 'receive', 'swap'] as const).map((f) => (
          <button
            key={f}
            onClick={() => setFilter(f)}
            className={`px-4 py-2 rounded-lg text-sm font-medium transition-colors capitalize ${
              filter === f
                ? 'bg-blue-500 text-white'
                : 'bg-[#1a1a2e] text-gray-300 hover:bg-[#2a2a3e]'
            }`}
          >
            {f}
          </button>
        ))}
      </div>

      {/* Transactions List */}
      <div className="bg-[#111] border border-[#2a2a35] rounded-2xl overflow-hidden">
        {filteredTransactions.map((tx) => (
          <TransactionItem key={tx.id} transaction={tx} />
        ))}
        {filteredTransactions.length === 0 && (
          <div className="p-8 text-center text-gray-400">
            No transactions found
          </div>
        )}
      </div>
    </div>
  );
}

function SettingsView({ network, onBack }: { network: Network; onBack: () => void }) {
  const [showMnemonic, setShowMnemonic] = useState(false);

  return (
    <div className="space-y-6 animate-in slide-in-from-right duration-500">
      <div className="flex items-center gap-4 mb-6">
        <button 
          onClick={onBack}
          className="p-2 bg-[#1a1a2e] hover:bg-[#2a2a3e] rounded-lg transition-colors"
        >
          <svg className="w-6 h-6 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
          </svg>
        </button>
        <h2 className="text-2xl font-bold text-white">Settings</h2>
      </div>

      {/* Network Settings */}
      <div className="bg-[#111] border border-[#2a2a35] rounded-2xl p-6">
        <h3 className="text-lg font-semibold text-white mb-4">Network</h3>
        <div className="space-y-4">
          <div>
            <label className="text-xs text-gray-400 uppercase tracking-wider mb-2 block">Current Network</label>
            <div className="flex items-center gap-3 p-3 bg-[#0a0a0f] rounded-lg">
              <Globe className={`w-5 h-5 ${network.isTestnet ? 'text-yellow-400' : 'text-green-400'}`} />
              <span className="text-white font-medium">{network.name}</span>
            </div>
          </div>
          <div>
            <label className="text-xs text-gray-400 uppercase tracking-wider mb-2 block">RPC URL</label>
            <input
              type="text"
              readOnly
              value={network.rpc}
              className="w-full bg-[#0a0a0f] border border-[#2a2a35] rounded-lg px-4 py-2 text-sm text-gray-300 font-mono"
            />
          </div>
        </div>
      </div>

      {/* Security Settings */}
      <div className="bg-[#111] border border-[#2a2a35] rounded-2xl p-6">
        <h3 className="text-lg font-semibold text-white mb-4">Security</h3>
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
            <label className="text-xs text-gray-400 uppercase tracking-wider mb-2 block">Recovery Phrase</label>
            <div className="bg-[#0a0a0f] border border-[#2a2a35] rounded-lg p-4">
              <div className="flex items-center justify-between mb-2">
                <span className="text-sm text-gray-300">
                  {showMnemonic ? 'test test test test test test test test test test test test' : '••••••••••••••••'}
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

      {/* About */}
      <div className="bg-[#111] border border-[#2a2a35] rounded-2xl p-6">
        <h3 className="text-lg font-semibold text-white mb-4">About</h3>
        <div className="space-y-2 text-sm text-gray-400">
          <div className="flex justify-between">
            <span>Version</span>
            <span className="text-white">1.0.0</span>
          </div>
          <div className="flex justify-between">
            <span>Network</span>
            <span className="text-white">{network.name}</span>
          </div>
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// Helper Components
// ============================================================================

function QuickAction({ icon, label, color, onClick }: { icon: React.ReactNode; label: string; color: string; onClick?: () => void }) {
  return (
    <button 
      onClick={onClick}
      className={`flex flex-col items-center justify-center gap-2 p-4 bg-gradient-to-br ${color} rounded-2xl shadow-lg transition-all hover:scale-105 hover:shadow-xl`}
    >
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
            {transaction.type === 'receive' ? <Receive className="w-4 h-4" /> :
             transaction.type === 'send' ? <Send className="w-4 h-4" /> :
             <ArrowLeftRight className="w-4 h-4" />}
          </div>
          <div>
            <div className="text-white font-medium capitalize">{transaction.type}</div>
            <div className="text-xs text-gray-400">
              {transaction.type === 'receive' ? 'From' : 'To'}: {transaction.from.slice(0, 10)}...
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

function NavButton({ icon, label, active, onClick }: { icon: React.ReactNode; label: string; active: boolean; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      className={`flex flex-col items-center gap-1 transition-colors ${
        active ? 'text-blue-400' : 'text-gray-400 hover:text-gray-300'
      }`}
    >
      {icon}
      <span className="text-[10px] font-medium">{label}</span>
    </button>
  );
}

function StatCard({ icon, label, value, trend, trendUp }: { icon: React.ReactNode; label: string; value: string; trend: string; trendUp: boolean }) {
  return (
    <div className="bg-[#111] border border-[#2a2a35] rounded-xl p-4">
      <div className="flex items-center gap-3 mb-2">
        <div className="p-2 bg-[#0a0a0f] rounded-lg">{icon}</div>
        <span className="text-sm text-gray-400">{label}</span>
      </div>
      <div className="flex items-end justify-between">
        <span className="text-2xl font-bold text-white">{value}</span>
        <span className={`text-xs font-medium ${trendUp ? 'text-green-400' : 'text-red-400'}`}>
          {trend}
        </span>
      </div>
    </div>
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
