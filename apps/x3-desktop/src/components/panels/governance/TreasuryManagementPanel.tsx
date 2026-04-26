import React, { useState } from "react";
import { Wallet2 as Wallet, TrendingUp, Zap, Eye, Download, Lock, AlertCircle } from "lucide-react";
import clsx from "clsx";

interface TreasuryAllocation {
  id: string;
  category: string;
  amount: number;
  percentage: number;
  status: "allocated" | "pending" | "spent";
  description: string;
}

interface MultiSigWallet {
  id: string;
  name: string;
  address: string;
  balance: number;
  signaturesRequired: number;
  signers: number;
  status: "active" | "inactive";
}

interface SpendingHistory {
  id: string;
  description: string;
  amount: number;
  recipient: string;
  date: string;
  approvers: number;
  status: "approved" | "pending" | "rejected";
}

const MOCK_ALLOCATIONS: TreasuryAllocation[] = [
  { id: "1", category: "Development", amount: 850000, percentage: 42.5, status: "allocated", description: "Core protocol development and upgrades" },
  { id: "2", category: "Marketing", amount: 380000, percentage: 19.0, status: "allocated", description: "Brand awareness and community growth" },
  { id: "3", category: "Grants & Bounties", amount: 520000, percentage: 26.0, status: "pending", description: "Developer incentives and bug bounties" },
  { id: "4", category: "Operations", amount: 250000, percentage: 12.5, status: "spent", description: "Infrastructure and administrative costs" },
];

const MOCK_WALLETS: MultiSigWallet[] = [
  {
    id: "1",
    name: "Core Treasury",
    address: "0xTreas...5234",
    balance: 2000000,
    signaturesRequired: 3,
    signers: 5,
    status: "active",
  },
  {
    id: "2",
    name: "Development Fund",
    address: "0xDev...8765",
    balance: 850000,
    signaturesRequired: 2,
    signers: 3,
    status: "active",
  },
  {
    id: "3",
    name: "Marketing Reserve",
    address: "0xMkt...1234",
    balance: 380000,
    signaturesRequired: 2,
    signers: 4,
    status: "active",
  },
];

const MOCK_SPENDING: SpendingHistory[] = [
  { id: "1", description: "Protocol Upgrade 2.0 Development", amount: 150000, recipient: "0xDev...1234", date: "2024-04-05", approvers: 4, status: "approved" },
  { id: "2", description: "Marketing Campaign Q2", amount: 75000, recipient: "0xMkt...5678", date: "2024-04-10", approvers: 3, status: "approved" },
  { id: "3", description: "Security Audit Services", amount: 50000, recipient: "0xSec...9abc", date: "2024-04-12", approvers: 2, status: "pending" },
];

export default function TreasuryManagementPanel() {
  const [allocations] = useState<TreasuryAllocation[]>(MOCK_ALLOCATIONS);
  const [wallets] = useState<MultiSigWallet[]>(MOCK_WALLETS);
  const [spending] = useState<SpendingHistory[]>(MOCK_SPENDING);
  const [activeTab, setActiveTab] = useState<"allocation" | "wallets" | "spending">("allocation");

  const totalTreasury = wallets.reduce((sum, w) => sum + w.balance, 0);
  const totalAllocated = allocations.reduce((sum, a) => sum + a.amount, 0);

  return (
    <div className="w-full h-full bg-[#0a0a0f] text-white p-6 flex flex-col">
      <h2 className="text-xl font-bold mb-4 flex items-center gap-2">
        <Wallet size={20} className="text-cyan-400" /> Treasury Management
      </h2>

      <div className="flex-1 overflow-y-auto space-y-4 mb-4">
        {/* Overview */}
        <div className="grid grid-cols-4 gap-2">
          <div className="bg-[#15151b] border border-[#2a2a35] rounded-lg p-3">
            <div className="text-xs text-gray-400 mb-1">Total Treasury</div>
            <div className="text-lg font-bold text-cyan-400">${(totalTreasury / 1000000).toFixed(2)}M</div>
          </div>
          <div className="bg-[#15151b] border border-[#2a2a35] rounded-lg p-3">
            <div className="text-xs text-gray-400 mb-1">Allocated</div>
            <div className="text-lg font-bold text-purple-400">${(totalAllocated / 1000000).toFixed(2)}M</div>
          </div>
          <div className="bg-[#15151b] border border-[#2a2a35] rounded-lg p-3">
            <div className="text-xs text-gray-400 mb-1">Multi-Sig Wallets</div>
            <div className="text-lg font-bold text-green-400">{wallets.length}</div>
          </div>
          <div className="bg-[#15151b] border border-[#2a2a35] rounded-lg p-3">
            <div className="text-xs text-gray-400 mb-1">Pending Approvals</div>
            <div className="text-lg font-bold text-orange-400">{spending.filter((s) => s.status === "pending").length}</div>
          </div>
        </div>

        {/* Tabs */}
        <div className="flex gap-2 border-b border-[#2a2a35]">
          {(["allocation", "wallets", "spending"] as const).map((tab) => (
            <button
              key={tab}
              onClick={() => setActiveTab(tab)}
              className={clsx(
                "px-4 py-2 text-sm font-semibold transition border-b-2 capitalize",
                activeTab === tab ? "border-cyan-600 text-cyan-400" : "border-transparent text-gray-400 hover:text-gray-300"
              )}
            >
              {tab === "allocation" ? "Budget" : tab}
            </button>
          ))}
        </div>

        {/* Budget Allocation */}
        {activeTab === "allocation" && (
          <div className="space-y-3">
            {allocations.map((alloc) => (
              <div key={alloc.id} className="bg-[#15151b] border border-[#2a2a35] rounded-lg p-3">
                <div className="flex justify-between items-start mb-2">
                  <div>
                    <div className="font-semibold text-sm">{alloc.category}</div>
                    <div className="text-xs text-gray-400">{alloc.description}</div>
                  </div>
                  <span
                    className={clsx(
                      "text-xs px-2 py-1 rounded font-bold",
                      alloc.status === "allocated" && "bg-blue-600/20 text-blue-400",
                      alloc.status === "pending" && "bg-yellow-600/20 text-yellow-400",
                      alloc.status === "spent" && "bg-green-600/20 text-green-400"
                    )}
                  >
                    {alloc.status}
                  </span>
                </div>

                <div className="bg-[#0a0a0f] rounded p-2 mb-2">
                  <div className="text-xs text-gray-400 mb-1">Budget: ${(alloc.amount / 1000000).toFixed(1)}M ({alloc.percentage}%)</div>
                  <div className="bg-[#2a2a35] rounded-full h-2">
                    <div
                      className={clsx("h-full rounded-full", alloc.status === "allocated" ? "bg-blue-600" : alloc.status === "pending" ? "bg-yellow-600" : "bg-green-600")}
                      style={{ width: `${alloc.percentage}%` }}
                    />
                  </div>
                </div>
              </div>
            ))}

            {/* Budget Summary */}
            <div className="bg-[#15151b] border border-[#2a2a35] rounded-lg p-3 mt-4">
              <div className="text-xs text-gray-400 mb-2 font-semibold">Budget Summary</div>
              <div className="space-y-1 text-xs">
                <div className="flex justify-between">
                  <span className="text-gray-400">Total Budget</span>
                  <span className="font-bold text-cyan-400">${(totalAllocated / 1000000).toFixed(2)}M</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-gray-400">Percentage of Treasury</span>
                  <span className="font-bold text-purple-400">{((totalAllocated / totalTreasury) * 100).toFixed(1)}%</span>
                </div>
              </div>
            </div>
          </div>
        )}

        {/* Multi-Sig Wallets */}
        {activeTab === "wallets" && (
          <div className="space-y-2">
            {wallets.map((wallet) => (
              <div key={wallet.id} className="bg-[#15151b] border border-[#2a2a35] rounded-lg p-3">
                <div className="flex justify-between items-start mb-2">
                  <div className="flex items-center gap-2">
                    <Lock size={16} className="text-green-400" />
                    <div>
                      <div className="font-semibold text-sm">{wallet.name}</div>
                      <div className="text-xs text-gray-400 font-mono">{wallet.address}</div>
                    </div>
                  </div>
                  <span className="text-xs px-2 py-1 bg-green-600/20 text-green-400 rounded font-bold">{wallet.status}</span>
                </div>

                <div className="grid grid-cols-3 gap-2 mb-2 text-xs">
                  <div>
                    <div className="text-gray-400">Balance</div>
                    <div className="font-bold text-cyan-400">${(wallet.balance / 1000000).toFixed(2)}M</div>
                  </div>
                  <div>
                    <div className="text-gray-400">Required Sigs</div>
                    <div className="font-bold text-purple-400">
                      {wallet.signaturesRequired}/{wallet.signers}
                    </div>
                  </div>
                  <div>
                    <div className="text-gray-400">% of Treasury</div>
                    <div className="font-bold text-orange-400">{((wallet.balance / totalTreasury) * 100).toFixed(1)}%</div>
                  </div>
                </div>

                <div className="text-xs text-gray-500">Multi-signature protection enabled</div>
              </div>
            ))}
          </div>
        )}

        {/* Spending History */}
        {activeTab === "spending" && (
          <div className="space-y-2">
            {spending.map((spend) => (
              <div key={spend.id} className="bg-[#15151b] border border-[#2a2a35] rounded-lg p-3">
                <div className="flex justify-between items-start mb-2">
                  <div className="flex-1">
                    <div className="flex items-center gap-2 mb-1">
                      <div className="font-semibold text-sm">{spend.description}</div>
                      <span
                        className={clsx(
                          "text-xs px-2 py-1 rounded font-bold",
                          spend.status === "approved" && "bg-green-600/20 text-green-400",
                          spend.status === "pending" && "bg-yellow-600/20 text-yellow-400",
                          spend.status === "rejected" && "bg-red-600/20 text-red-400"
                        )}
                      >
                        {spend.status}
                      </span>
                    </div>
                    <div className="text-xs text-gray-400">{spend.date}</div>
                  </div>
                  <div className="text-right">
                    <div className="font-bold text-cyan-400">${(spend.amount / 1000).toFixed(0)}K</div>
                  </div>
                </div>

                <div className="grid grid-cols-2 gap-2 text-xs mb-2">
                  <div>
                    <div className="text-gray-400">Recipient</div>
                    <div className="font-mono text-gray-500 text-xs">{spend.recipient}</div>
                  </div>
                  <div>
                    <div className="text-gray-400">Approvals</div>
                    <div className="font-bold text-purple-400">{spend.approvers} signatures</div>
                  </div>
                </div>

                {spend.status === "pending" && (
                  <div className="flex gap-2">
                    <button className="flex-1 bg-green-600/20 text-green-400 text-xs font-semibold py-1 rounded hover:bg-green-600/30">Approve</button>
                    <button className="flex-1 bg-red-600/20 text-red-400 text-xs font-semibold py-1 rounded hover:bg-red-600/30">Reject</button>
                  </div>
                )}
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="text-xs text-gray-500 text-center pt-4 border-t border-[#2a2a35]">
        Multi-signature treasury, budget allocation, spending history, and approval workflows.
      </div>
    </div>
  );
}
