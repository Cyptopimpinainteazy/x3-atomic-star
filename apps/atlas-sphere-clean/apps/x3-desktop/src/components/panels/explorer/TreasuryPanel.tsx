import { useState, useEffect } from "react";
import {
  DollarSign,
  TrendingUp,
  PieChart,
  Activity,
  ArrowUp,
  RefreshCw,
  CheckCircle,
  Clock,
  Zap,
  Shield,
  Users,
} from "lucide-react";

const treasuryMetrics = [
  { label: "Total Treasury", value: "$89.0M", change: "+12.4%", icon: DollarSign, color: "text-green-400" },
  { label: "Daily Revenue", value: "$127K", change: "+8.7%", icon: TrendingUp, color: "text-blue-400" },
  { label: "Active Distributions", value: "6", change: "", icon: Activity, color: "text-purple-400" },
  { label: "DAO Participation", value: "94.7%", change: "", icon: Users, color: "text-cyan-400" },
  { label: "Burn Rate", value: "$340K", change: "", icon: Zap, color: "text-orange-400" },
  { label: "Auto Distribution", value: "100%", change: "", icon: Shield, color: "text-emerald-400" },
];

const feeDistribution = [
  { label: "DAO Treasury", pct: 40, amount: "$35.6M", color: "bg-purple-500" },
  { label: "Development", pct: 20, amount: "$17.8M", color: "bg-blue-500" },
  { label: "Marketing", pct: 10, amount: "$8.9M", color: "bg-cyan-500" },
  { label: "Liquidity", pct: 15, amount: "$13.35M", color: "bg-green-500" },
  { label: "Buyback & Burn", pct: 10, amount: "$8.9M", color: "bg-orange-500" },
  { label: "Insurance Fund", pct: 5, amount: "$4.45M", color: "bg-red-500" },
];

const recentTransactions = [
  { type: "distribution", desc: "DAO Distribution Q4", amount: "+$2.4M", time: "2 hours ago", status: "completed" },
  { type: "burn", desc: "Token Buyback & Burn", amount: "-$340K", time: "6 hours ago", status: "completed" },
  { type: "revenue", desc: "Protocol Fee Revenue", amount: "+$127K", time: "12 hours ago", status: "completed" },
  { type: "distribution", desc: "Dev Fund Allocation", amount: "+$890K", time: "1 day ago", status: "completed" },
  { type: "insurance", desc: "Insurance Pool Top-up", amount: "+$445K", time: "2 days ago", status: "pending" },
];

const revenueStreams = [
  { protocol: "X3 Swap", revenue: "$48.2K/day", feeRate: "0.3%", volume: "$16.1M", growth: "+15.3%" },
  { protocol: "X3 Bridge", revenue: "$32.1K/day", feeRate: "0.1%", volume: "$32.1M", growth: "+22.8%" },
  { protocol: "X3 Lend", revenue: "$28.7K/day", feeRate: "0.5%", volume: "$5.7M", growth: "+8.4%" },
  { protocol: "X3 Perps", revenue: "$18.0K/day", feeRate: "0.05%", volume: "$36.0M", growth: "+31.2%" },
];

export default function TreasuryPanel() {
  const [lastUpdated, setLastUpdated] = useState(new Date());

  useEffect(() => {
    const interval = setInterval(() => setLastUpdated(new Date()), 30000);
    return () => clearInterval(interval);
  }, []);

  return (
    <div className="overflow-y-auto h-full bg-slate-900 text-white p-6 space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold bg-gradient-to-r from-purple-400 to-blue-400 bg-clip-text text-transparent">
            Treasury Management
          </h1>
          <p className="text-sm text-slate-400 mt-1">
            Auto-refresh every 30s · Updated {lastUpdated.toLocaleTimeString()}
          </p>
        </div>
        <button
          onClick={() => setLastUpdated(new Date())}
          className="p-2 rounded-lg bg-slate-800 text-slate-400 hover:text-white transition-colors"
        >
          <RefreshCw className="w-4 h-4" />
        </button>
      </div>

      {/* Metrics Cards */}
      <div className="grid grid-cols-3 gap-4">
        {treasuryMetrics.map((m) => (
          <div
            key={m.label}
            className="bg-slate-800/60 border border-slate-700/50 rounded-xl p-4 hover:border-purple-500/30 transition-colors"
          >
            <div className="flex items-center justify-between mb-2">
              <span className="text-xs text-slate-400">{m.label}</span>
              <m.icon className={`w-4 h-4 ${m.color}`} />
            </div>
            <div className="text-xl font-bold">{m.value}</div>
            {m.change && (
              <div className="flex items-center gap-1 mt-1">
                <ArrowUp className="w-3 h-3 text-green-400" />
                <span className="text-xs text-green-400">{m.change}</span>
              </div>
            )}
          </div>
        ))}
      </div>

      {/* Fee Distribution */}
      <div className="bg-slate-800/40 border border-slate-700/50 rounded-xl p-5">
        <div className="flex items-center gap-2 mb-4">
          <PieChart className="w-5 h-5 text-purple-400" />
          <h2 className="text-lg font-semibold">Fee Distribution</h2>
        </div>
        <div className="space-y-3">
          {feeDistribution.map((d) => (
            <div key={d.label} className="space-y-1">
              <div className="flex items-center justify-between text-sm">
                <span className="text-slate-300">{d.label}</span>
                <div className="flex items-center gap-3">
                  <span className="text-slate-400">{d.amount}</span>
                  <span className="text-slate-500 w-10 text-right">{d.pct}%</span>
                </div>
              </div>
              <div className="w-full bg-slate-700/50 rounded-full h-2 overflow-hidden">
                <div
                  className={`${d.color} h-full rounded-full transition-all`}
                  style={{ width: `${d.pct}%` }}
                />
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* Recent Transactions */}
      <div className="bg-slate-800/40 border border-slate-700/50 rounded-xl p-5">
        <div className="flex items-center gap-2 mb-4">
          <Activity className="w-5 h-5 text-blue-400" />
          <h2 className="text-lg font-semibold">Recent Transactions</h2>
        </div>
        <div className="space-y-3">
          {recentTransactions.map((tx, i) => (
            <div
              key={i}
              className="flex items-center justify-between bg-slate-900/50 rounded-lg p-3"
            >
              <div className="flex items-center gap-3">
                <div
                  className={`w-8 h-8 rounded-lg flex items-center justify-center ${
                    tx.type === "distribution"
                      ? "bg-purple-500/20"
                      : tx.type === "burn"
                      ? "bg-orange-500/20"
                      : tx.type === "revenue"
                      ? "bg-green-500/20"
                      : "bg-blue-500/20"
                  }`}
                >
                  {tx.type === "distribution" ? (
                    <Users className="w-4 h-4 text-purple-400" />
                  ) : tx.type === "burn" ? (
                    <Zap className="w-4 h-4 text-orange-400" />
                  ) : tx.type === "revenue" ? (
                    <DollarSign className="w-4 h-4 text-green-400" />
                  ) : (
                    <Shield className="w-4 h-4 text-blue-400" />
                  )}
                </div>
                <div>
                  <div className="text-sm font-medium">{tx.desc}</div>
                  <div className="text-xs text-slate-500">{tx.time}</div>
                </div>
              </div>
              <div className="text-right">
                <div
                  className={`text-sm font-semibold ${
                    tx.amount.startsWith("+") ? "text-green-400" : "text-orange-400"
                  }`}
                >
                  {tx.amount}
                </div>
                <div className="flex items-center gap-1 justify-end">
                  {tx.status === "completed" ? (
                    <CheckCircle className="w-3 h-3 text-green-500" />
                  ) : (
                    <Clock className="w-3 h-3 text-yellow-500" />
                  )}
                  <span className="text-xs text-slate-500 capitalize">{tx.status}</span>
                </div>
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* Revenue Streams */}
      <div className="bg-slate-800/40 border border-slate-700/50 rounded-xl p-5">
        <div className="flex items-center gap-2 mb-4">
          <TrendingUp className="w-5 h-5 text-green-400" />
          <h2 className="text-lg font-semibold">Revenue Streams</h2>
        </div>
        <div className="grid grid-cols-2 gap-3">
          {revenueStreams.map((r) => (
            <div key={r.protocol} className="bg-slate-900/50 rounded-lg p-4">
              <div className="text-sm font-semibold mb-2">{r.protocol}</div>
              <div className="space-y-1.5 text-xs">
                <div className="flex justify-between">
                  <span className="text-slate-400">Revenue</span>
                  <span className="text-green-400">{r.revenue}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-slate-400">Fee Rate</span>
                  <span className="text-slate-300">{r.feeRate}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-slate-400">Volume</span>
                  <span className="text-slate-300">{r.volume}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-slate-400">Growth</span>
                  <span className="text-green-400">{r.growth}</span>
                </div>
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
