import React, { useState } from "react";
import { Vote, TrendingUp, Zap, Eye, Download, CheckCircle, AlertCircle } from "lucide-react";
import clsx from "clsx";

interface GovernanceProposal {
  id: string;
  title: string;
  description: string;
  proposer: string;
  votesFor: number;
  votesAgainst: number;
  votesAbstain: number;
  status: "active" | "passed" | "rejected" | "executed";
  deadline: string;
  quorumRequired: number;
}

interface VoteBreakdown {
  for: number;
  against: number;
  abstain: number;
}

interface GovernanceMetrics {
  totalVoters: number;
  activeDaos: number;
  totalProposals: number;
  avgParticipation: number;
}

const MOCK_PROPOSALS: GovernanceProposal[] = [
  {
    id: "1",
    title: "Increase Community Fund Size to 50M X3",
    description: "Proposal to increase the community development fund from 30M to 50M X3 to accelerate ecosystem growth.",
    proposer: "0xGov...5234",
    votesFor: 8500000,
    votesAgainst: 1200000,
    votesAbstain: 300000,
    status: "active",
    deadline: "2024-04-20",
    quorumRequired: 8000000,
  },
  {
    id: "2",
    title: "Launch Strategic Partnership with ChainLink",
    description: "Establish official partnership for oracle integration and data feeds across the X3 ecosystem.",
    proposer: "0xBiz...1234",
    votesFor: 12500000,
    votesAgainst: 800000,
    votesAbstain: 500000,
    status: "passed",
    deadline: "2024-04-10",
    quorumRequired: 8000000,
  },
  {
    id: "3",
    title: "Implement Dynamic Staking Rewards",
    description: "Change staking reward mechanism to dynamic model based on network traffic and TVL.",
    proposer: "0xTech...8765",
    votesFor: 6200000,
    votesAgainst: 4100000,
    votesAbstain: 200000,
    status: "rejected",
    deadline: "2024-04-05",
    quorumRequired: 8000000,
  },
];

const METRICS: GovernanceMetrics = {
  totalVoters: 45234,
  activeDaos: 12,
  totalProposals: 87,
  avgParticipation: 68,
};

export default function GovernanceProposalsPanel() {
  const [proposals] = useState<GovernanceProposal[]>(MOCK_PROPOSALS);
  const [selectedProposal, setSelectedProposal] = useState<GovernanceProposal | null>(proposals[0]);
  const [activeTab, setActiveTab] = useState<"proposals" | "details">("proposals");

  const activeProposals = proposals.filter((p) => p.status === "active").length;
  const totalVotes = selectedProposal ? selectedProposal.votesFor + selectedProposal.votesAgainst + selectedProposal.votesAbstain : 0;

  return (
    <div className="w-full h-full bg-[#0a0a0f] text-white p-6 flex flex-col">
      <h2 className="text-xl font-bold mb-4 flex items-center gap-2">
        <Vote size={20} className="text-blue-400" /> Governance
      </h2>

      <div className="flex-1 overflow-y-auto space-y-4 mb-4">
        {/* Overview */}
        <div className="grid grid-cols-4 gap-2">
          <div className="bg-[#15151b] border border-[#2a2a35] rounded-lg p-3">
            <div className="text-xs text-gray-400 mb-1">Active Proposals</div>
            <div className="text-lg font-bold text-blue-400">{activeProposals}</div>
          </div>
          <div className="bg-[#15151b] border border-[#2a2a35] rounded-lg p-3">
            <div className="text-xs text-gray-400 mb-1">Total Proposals</div>
            <div className="text-lg font-bold text-purple-400">{METRICS.totalProposals}</div>
          </div>
          <div className="bg-[#15151b] border border-[#2a2a35] rounded-lg p-3">
            <div className="text-xs text-gray-400 mb-1">Active Voters</div>
            <div className="text-lg font-bold text-green-400">{METRICS.totalVoters.toLocaleString()}</div>
          </div>
          <div className="bg-[#15151b] border border-[#2a2a35] rounded-lg p-3">
            <div className="text-xs text-gray-400 mb-1">Avg Participation</div>
            <div className="text-lg font-bold text-orange-400">{METRICS.avgParticipation}%</div>
          </div>
        </div>

        {/* Tabs */}
        <div className="flex gap-2 border-b border-[#2a2a35]">
          {(["proposals", "details"] as const).map((tab) => (
            <button
              key={tab}
              onClick={() => setActiveTab(tab)}
              className={clsx(
                "px-4 py-2 text-sm font-semibold transition border-b-2 capitalize",
                activeTab === tab ? "border-blue-600 text-blue-400" : "border-transparent text-gray-400 hover:text-gray-300"
              )}
            >
              {tab}
            </button>
          ))}
        </div>

        {/* Proposals List */}
        {activeTab === "proposals" && (
          <div className="space-y-2">
            {proposals.map((proposal) => {
              const statusColor = proposal.status === "active" ? "yellow" : proposal.status === "passed" ? "green" : "red";
              const totalVotes = proposal.votesFor + proposal.votesAgainst + proposal.votesAbstain;
              const approvalPct = (proposal.votesFor / totalVotes) * 100;

              return (
                <div
                  key={proposal.id}
                  onClick={() => {
                    setSelectedProposal(proposal);
                    setActiveTab("details");
                  }}
                  className={clsx("bg-[#15151b] border rounded-lg p-3 cursor-pointer transition", selectedProposal?.id === proposal.id ? "border-blue-600" : "border-[#2a2a35] hover:border-blue-600/50")}
                >
                  <div className="flex justify-between items-start mb-2">
                    <div className="flex-1">
                      <div className="flex items-center gap-2 mb-1">
                        <div className="font-semibold text-sm">{proposal.title}</div>
                        <span
                          className={clsx(
                            "text-xs px-2 py-1 rounded font-bold",
                            proposal.status === "active" && "bg-yellow-600/20 text-yellow-400",
                            proposal.status === "passed" && "bg-green-600/20 text-green-400",
                            proposal.status === "rejected" && "bg-red-600/20 text-red-400",
                            proposal.status === "executed" && "bg-blue-600/20 text-blue-400"
                          )}
                        >
                          {proposal.status}
                        </span>
                      </div>
                      <div className="text-xs text-gray-400">{proposal.description.slice(0, 60)}...</div>
                    </div>
                    {proposal.status === "passed" && <CheckCircle size={16} className="text-green-400 flex-shrink-0" />}
                    {proposal.status === "rejected" && <AlertCircle size={16} className="text-red-400 flex-shrink-0" />}
                  </div>

                  <div className="bg-[#0a0a0f] rounded p-2 mb-2">
                    <div className="flex-1 bg-[#2a2a35] rounded-full h-2 flex overflow-hidden">
                      <div className="bg-green-600 h-full" style={{ width: `${approvalPct}%` }} />
                      <div className="bg-red-600 h-full" style={{ width: `${(proposal.votesAgainst / totalVotes) * 100}%` }} />
                      <div className="bg-gray-600 h-full" style={{ width: `${(proposal.votesAbstain / totalVotes) * 100}%` }} />
                    </div>
                  </div>

                  <div className="grid grid-cols-3 gap-2 text-xs">
                    <div>
                      <div className="text-green-400 font-bold">{approvalPct.toFixed(1)}% For</div>
                      <div className="text-gray-500">{(proposal.votesFor / 1000000).toFixed(1)}M X3</div>
                    </div>
                    <div>
                      <div className="text-red-400 font-bold">{((proposal.votesAgainst / totalVotes) * 100).toFixed(1)}% Against</div>
                      <div className="text-gray-500">{(proposal.votesAgainst / 1000000).toFixed(1)}M X3</div>
                    </div>
                    <div>
                      <div className="text-gray-500 font-bold">{((proposal.votesAbstain / totalVotes) * 100).toFixed(1)}% Abstain</div>
                      <div className="text-gray-500">{(proposal.votesAbstain / 1000000).toFixed(1)}M X3</div>
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        )}

        {/* Proposal Details */}
        {activeTab === "details" && selectedProposal && (
          <div className="space-y-3">
            {/* Full Details */}
            <div className="bg-[#15151b] border border-[#2a2a35] rounded-lg p-4 space-y-3">
              <div>
                <h3 className="text-sm font-semibold mb-1">{selectedProposal.title}</h3>
                <p className="text-xs text-gray-400 leading-relaxed">{selectedProposal.description}</p>
              </div>

              <div className="grid grid-cols-2 gap-2 text-xs pb-3 border-b border-[#2a2a35]">
                <div>
                  <div className="text-gray-400">Proposer</div>
                  <div className="font-mono text-gray-500">{selectedProposal.proposer}</div>
                </div>
                <div>
                  <div className="text-gray-400">Deadline</div>
                  <div className="font-semibold text-cyan-400">{selectedProposal.deadline}</div>
                </div>
              </div>
            </div>

            {/* Vote Breakdown */}
            <div className="bg-[#15151b] border border-[#2a2a35] rounded-lg p-4">
              <div className="text-xs text-gray-400 mb-3 font-semibold">Vote Breakdown</div>

              <div className="space-y-3 mb-3">
                {/* For */}
                <div>
                  <div className="flex justify-between mb-1 text-xs">
                    <span className="text-green-400">For</span>
                    <span className="font-bold text-cyan-400">{(selectedProposal.votesFor / 1000000).toFixed(1)}M X3</span>
                  </div>
                  <div className="bg-[#2a2a35] rounded-full h-2">
                    <div className="h-full bg-green-600 rounded-full" style={{ width: `${(selectedProposal.votesFor / (selectedProposal.votesFor + selectedProposal.votesAgainst + selectedProposal.votesAbstain)) * 100}%` }} />
                  </div>
                </div>

                {/* Against */}
                <div>
                  <div className="flex justify-between mb-1 text-xs">
                    <span className="text-red-400">Against</span>
                    <span className="font-bold text-cyan-400">{(selectedProposal.votesAgainst / 1000000).toFixed(1)}M X3</span>
                  </div>
                  <div className="bg-[#2a2a35] rounded-full h-2">
                    <div className="h-full bg-red-600 rounded-full" style={{ width: `${(selectedProposal.votesAgainst / (selectedProposal.votesFor + selectedProposal.votesAgainst + selectedProposal.votesAbstain)) * 100}%` }} />
                  </div>
                </div>

                {/* Abstain */}
                <div>
                  <div className="flex justify-between mb-1 text-xs">
                    <span className="text-gray-400">Abstain</span>
                    <span className="font-bold text-cyan-400">{(selectedProposal.votesAbstain / 1000000).toFixed(1)}M X3</span>
                  </div>
                  <div className="bg-[#2a2a35] rounded-full h-2">
                    <div className="h-full bg-gray-600 rounded-full" style={{ width: `${(selectedProposal.votesAbstain / (selectedProposal.votesFor + selectedProposal.votesAgainst + selectedProposal.votesAbstain)) * 100}%` }} />
                  </div>
                </div>
              </div>

              <div className="grid grid-cols-2 gap-2 text-xs pt-3 border-t border-[#2a2a35]">
                <div>
                  <div className="text-gray-400">Quorum Required</div>
                  <div className="font-bold text-cyan-400">{(selectedProposal.quorumRequired / 1000000).toFixed(1)}M X3</div>
                </div>
                <div>
                  <div className="text-gray-400">Total Votes</div>
                  <div className="font-bold text-cyan-400">{((selectedProposal.votesFor + selectedProposal.votesAgainst + selectedProposal.votesAbstain) / 1000000).toFixed(1)}M X3</div>
                </div>
              </div>
            </div>

            {/* Action Button */}
            <div className="flex gap-2">
              <button className="flex-1 bg-green-600/20 text-green-400 text-sm font-semibold py-2 rounded hover:bg-green-600/30">Vote For</button>
              <button className="flex-1 bg-red-600/20 text-red-400 text-sm font-semibold py-2 rounded hover:bg-red-600/30">Vote Against</button>
              <button className="flex-1 bg-gray-600/20 text-gray-400 text-sm font-semibold py-2 rounded hover:bg-gray-600/30">Abstain</button>
            </div>
          </div>
        )}
      </div>

      <div className="text-xs text-gray-500 text-center pt-4 border-t border-[#2a2a35]">
        DAO proposals, voting mechanics, quorum tracking, and proposal timeline.
      </div>
    </div>
  );
}
