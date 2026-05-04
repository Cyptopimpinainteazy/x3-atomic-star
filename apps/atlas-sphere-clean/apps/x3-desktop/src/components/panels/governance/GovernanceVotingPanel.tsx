import React, { useState } from "react";
import { Vote, CheckCircle, Clock, TrendingUp, BarChart3, AlertCircle } from "lucide-react";
import clsx from "clsx";

interface Proposal {
  id: string;
  title: string;
  description: string;
  type: "parameter" | "upgrade" | "treasury" | "other";
  status: "active" | "passed" | "failed" | "pending";
  yesVotes: number;
  noVotes: number;
  abstainVotes: number;
  quorumReq: number;
  timeRemaining: string;
  proposer: string;
  createdAt: string;
}

interface Vote {
  id: string;
  proposalId: string;
  voter: string;
  vote: "yes" | "no" | "abstain";
  power: number;
  timestamp: string;
}

const MOCK_PROPOSALS: Proposal[] = [
  {
    id: "prop-001",
    title: "Increase Validator Stake Requirement",
    description: "Proposal to increase minimum stake from 100 X3 to 250 X3 for network security",
    type: "parameter",
    status: "active",
    yesVotes: 15240000,
    noVotes: 2150000,
    abstainVotes: 850000,
    quorumReq: 15000000,
    timeRemaining: "2d 5h 30m",
    proposer: "0x7a2c...8f4d",
    createdAt: "2024-01-12",
  },
  {
    id: "prop-002",
    title: "Deploy Protocol v2.1 Upgrade",
    description: "Upgrade runtime to support next-gen consensus mechanism with improved throughput",
    type: "upgrade",
    status: "active",
    yesVotes: 12800000,
    noVotes: 3200000,
    abstainVotes: 440000,
    quorumReq: 15000000,
    timeRemaining: "1d 3h",
    proposer: "0x9b1e...2c7a",
    createdAt: "2024-01-14",
  },
  {
    id: "prop-003",
    title: "Treasury Allocation for Research",
    description: "Allocate 500K X3 from treasury to research initiatives on MEV mitigation",
    type: "treasury",
    status: "passed",
    yesVotes: 18500000,
    noVotes: 1200000,
    abstainVotes: 300000,
    quorumReq: 15000000,
    timeRemaining: "Passed",
    proposer: "0x4c5d...9e2f",
    createdAt: "2024-01-10",
  },
  {
    id: "prop-004",
    title: "Community Governance Framework Update",
    description: "New voting mechanism with delegation support and timelock improvements",
    type: "other",
    status: "pending",
    yesVotes: 0,
    noVotes: 0,
    abstainVotes: 0,
    quorumReq: 15000000,
    timeRemaining: "Voting Soon",
    proposer: "0xd3e2...1a5b",
    createdAt: "2024-01-15",
  },
];

const MOCK_VOTES: Vote[] = [
  {
    id: "vote-001",
    proposalId: "prop-001",
    voter: "0x7a2c...8f4d",
    vote: "yes",
    power: 5000000,
    timestamp: "1h ago",
  },
  {
    id: "vote-002",
    proposalId: "prop-001",
    voter: "0x9b1e...2c7a",
    vote: "yes",
    power: 4200000,
    timestamp: "2h ago",
  },
  {
    id: "vote-003",
    proposalId: "prop-002",
    voter: "0x4c5d...9e2f",
    vote: "no",
    power: 2100000,
    timestamp: "30m ago",
  },
];

type TabType = "active" | "voted" | "create" | "history";

export default function GovernanceVotingPanel() {
  const [activeTab, setActiveTab] = useState<TabType>("active");
  const [proposals, setProposals] = useState<Proposal[]>(MOCK_PROPOSALS);
  const [selectedProposal, setSelectedProposal] = useState<string | null>(null);
  const [userVotingPower] = useState(3500000);

  const activeProposals = proposals.filter((p) => p.status === "active" || p.status === "pending");

  const handleVote = (proposalId: string, voteType: "yes" | "no" | "abstain") => {
    setProposals(
      proposals.map((p) => {
        if (p.id === proposalId) {
          const updated = { ...p };
          switch (voteType) {
            case "yes":
              updated.yesVotes += userVotingPower;
              break;
            case "no":
              updated.noVotes += userVotingPower;
              break;
            case "abstain":
              updated.abstainVotes += userVotingPower;
              break;
          }
          return updated;
        }
        return p;
      })
    );
  };

  const getStatusColor = (status: string) => {
    if (status === "passed") return "bg-green-600/20 text-green-400";
    if (status === "failed") return "bg-red-600/20 text-red-400";
    if (status === "pending") return "bg-yellow-600/20 text-yellow-400";
    return "bg-cyan-600/20 text-cyan-400";
  };

  const getProposalTypeColor = (type: string) => {
    const colors: Record<string, string> = {
      parameter: "bg-blue-600/20 text-blue-400",
      upgrade: "bg-purple-600/20 text-purple-400",
      treasury: "bg-green-600/20 text-green-400",
      other: "bg-gray-600/20 text-gray-400",
    };
    return colors[type] || colors.other;
  };

  const calculatePassingChance = (yes: number, no: number) => {
    const total = yes + no;
    if (total === 0) return 0;
    return (yes / total) * 100;
  };

  return (
    <div className="w-full h-full bg-[#0a0a0f] text-white p-6 flex flex-col">
      <h2 className="text-xl font-bold mb-4 flex items-center gap-2">
        <Vote size={20} className="text-purple-400" /> Governance & Voting
      </h2>

      {/* Tab Navigation */}
      <div className="flex gap-2 mb-4 border-b border-[#2a2a35] overflow-x-auto">
        {(["active", "voted", "create", "history"] as const).map((tab) => (
          <button
            key={tab}
            onClick={() => setActiveTab(tab)}
            className={clsx(
              "px-4 py-2 text-sm font-semibold border-b-2 transition whitespace-nowrap",
              activeTab === tab
                ? "border-purple-400 text-purple-400"
                : "border-transparent text-gray-400 hover:text-white"
            )}
          >
            {tab === "active" && `Active (${activeProposals.length})`}
            {tab === "voted" && `My Votes (${MOCK_VOTES.length})`}
            {tab === "create" && "Create Proposal"}
            {tab === "history" && "History"}
          </button>
        ))}
      </div>

      <div className="flex-1 overflow-y-auto space-y-4">
        {/* Active Proposals */}
        {activeTab === "active" && (
          <div className="space-y-3">
            {activeProposals.map((proposal) => {
              const totalVotes = proposal.yesVotes + proposal.noVotes + proposal.abstainVotes;
              const passingChance = calculatePassingChance(proposal.yesVotes, proposal.noVotes);
              const quorumMet = totalVotes >= proposal.quorumReq;

              return (
                <div
                  key={proposal.id}
                  className="bg-[#15151b] border border-[#2a2a35] rounded-lg p-4 hover:border-[#3a3a45] transition cursor-pointer"
                  onClick={() => setSelectedProposal(selectedProposal === proposal.id ? null : proposal.id)}
                >
                  <div className="flex justify-between items-start mb-2">
                    <div>
                      <h3 className="font-semibold text-sm">{proposal.title}</h3>
                      <div className="flex gap-2 mt-2">
                        <span className={clsx("text-xs px-2 py-0.5 rounded", getStatusColor(proposal.status))}>
                          {proposal.status.charAt(0).toUpperCase() + proposal.status.slice(1)}
                        </span>
                        <span className={clsx("text-xs px-2 py-0.5 rounded", getProposalTypeColor(proposal.type))}>
                          {proposal.type.charAt(0).toUpperCase() + proposal.type.slice(1)}
                        </span>
                      </div>
                    </div>
                    <div className="text-right text-xs">
                      <div className="text-gray-500 flex items-center gap-1 justify-end">
                        <Clock size={12} /> {proposal.timeRemaining}
                      </div>
                    </div>
                  </div>

                  {/* Vote Progress */}
                  <div className="mt-3 space-y-2">
                    <div className="flex gap-2">
                      <div className="flex-1">
                        <div className="text-xs text-gray-500 mb-1">Yes: {(proposal.yesVotes / 1000000).toFixed(1)}M</div>
                        <div className="bg-[#0a0a0f] rounded-full h-2">
                          <div
                            className="h-full bg-green-600 rounded-full"
                            style={{ width: `${(proposal.yesVotes / (proposal.yesVotes + proposal.noVotes)) * 100 || 0}%` }}
                          />
                        </div>
                      </div>
                      <div className="flex-1">
                        <div className="text-xs text-gray-500 mb-1">No: {(proposal.noVotes / 1000000).toFixed(1)}M</div>
                        <div className="bg-[#0a0a0f] rounded-full h-2">
                          <div
                            className="h-full bg-red-600 rounded-full"
                            style={{ width: `${(proposal.noVotes / (proposal.yesVotes + proposal.noVotes)) * 100 || 0}%` }}
                          />
                        </div>
                      </div>
                    </div>
                  </div>

                  {/* Quorum Check */}
                  <div className="mt-2 text-xs flex items-center gap-1">
                    {quorumMet ? (
                      <>
                        <CheckCircle size={12} className="text-green-400" />
                        <span className="text-green-400">Quorum met</span>
                      </>
                    ) : (
                      <>
                        <AlertCircle size={12} className="text-yellow-400" />
                        <span className="text-yellow-400">
                          {(proposal.quorumReq - totalVotes) / 1000000 > 0
                            ? `${((proposal.quorumReq - totalVotes) / 1000000).toFixed(1)}M needed for quorum`
                            : "Quorum reached"}
                        </span>
                      </>
                    )}
                  </div>

                  {/* Expanded Details */}
                  {selectedProposal === proposal.id && (
                    <div className="mt-4 pt-4 border-t border-[#2a2a35] space-y-3">
                      <p className="text-xs text-gray-400 leading-relaxed">{proposal.description}</p>
                      <div className="flex justify-between text-xs">
                        <span className="text-gray-500">Proposer:</span>
                        <span className="text-cyan-400 font-mono">{proposal.proposer}</span>
                      </div>
                      <div className="flex justify-between text-xs">
                        <span className="text-gray-500">Passing Chance:</span>
                        <span className="text-yellow-400 font-mono">{passingChance.toFixed(1)}%</span>
                      </div>

                      {proposal.status === "active" && (
                        <div className="flex gap-2 mt-3">
                          <button
                            onClick={() => handleVote(proposal.id, "yes")}
                            className="flex-1 bg-green-600/20 border border-green-600 text-green-400 py-1.5 rounded text-xs font-semibold hover:bg-green-600/30 transition"
                          >
                            Vote Yes
                          </button>
                          <button
                            onClick={() => handleVote(proposal.id, "no")}
                            className="flex-1 bg-red-600/20 border border-red-600 text-red-400 py-1.5 rounded text-xs font-semibold hover:bg-red-600/30 transition"
                          >
                            Vote No
                          </button>
                          <button
                            onClick={() => handleVote(proposal.id, "abstain")}
                            className="flex-1 bg-gray-600/20 border border-gray-600 text-gray-400 py-1.5 rounded text-xs font-semibold hover:bg-gray-600/30 transition"
                          >
                            Abstain
                          </button>
                        </div>
                      )}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}

        {/* My Votes Tab */}
        {activeTab === "voted" && (
          <div className="space-y-3">
            <div className="bg-[#15151b] border border-[#2a2a35] rounded-lg p-3 text-xs">
              <div className="text-gray-500 mb-1">Voting Power</div>
              <div className="text-lg font-bold text-purple-400">{(userVotingPower / 1000000).toFixed(1)}M X3</div>
            </div>
            {MOCK_VOTES.map((vote) => {
              const proposal = proposals.find((p) => p.id === vote.proposalId);
              return (
                <div key={vote.id} className="bg-[#15151b] border border-[#2a2a35] rounded-lg p-3">
                  <div className="flex justify-between items-start">
                    <div>
                      <div className="text-sm font-semibold">{proposal?.title}</div>
                      <div className="text-xs text-gray-500 mt-1">Power: {(vote.power / 1000000).toFixed(1)}M X3</div>
                      <div className="text-xs text-gray-600 mt-1">{vote.timestamp}</div>
                    </div>
                    <span
                      className={clsx(
                        "text-xs px-2 py-1 rounded font-semibold",
                        vote.vote === "yes"
                          ? "bg-green-600/20 text-green-400"
                          : vote.vote === "no"
                          ? "bg-red-600/20 text-red-400"
                          : "bg-gray-600/20 text-gray-400"
                      )}
                    >
                      {vote.vote.toUpperCase()}
                    </span>
                  </div>
                </div>
              );
            })}
          </div>
        )}

        {/* Create Proposal */}
        {activeTab === "create" && (
          <div className="bg-[#15151b] border border-[#2a2a35] rounded-lg p-4 space-y-4 max-w-md">
            <h3 className="font-semibold text-sm">Create New Proposal</h3>
            <div>
              <label className="text-xs text-gray-400">Title</label>
              <input
                type="text"
                placeholder="Proposal title"
                className="w-full mt-1 bg-[#0a0a0f] border border-[#2a2a35] rounded px-3 py-2 text-sm text-white placeholder-gray-600"
              />
            </div>
            <div>
              <label className="text-xs text-gray-400">Description</label>
              <textarea
                placeholder="Detailed description"
                className="w-full mt-1 bg-[#0a0a0f] border border-[#2a2a35] rounded px-3 py-2 text-sm text-white placeholder-gray-600 h-24"
              />
            </div>
            <div>
              <label className="text-xs text-gray-400">Type</label>
              <select className="w-full mt-1 bg-[#0a0a0f] border border-[#2a2a35] rounded px-3 py-2 text-sm text-white">
                <option value="parameter">Parameter</option>
                <option value="upgrade">Upgrade</option>
                <option value="treasury">Treasury</option>
                <option value="other">Other</option>
              </select>
            </div>
            <button className="w-full bg-purple-600/20 border border-purple-600 text-purple-400 py-2 rounded font-semibold text-sm hover:bg-purple-600/30 transition">
              Submit Proposal
            </button>
          </div>
        )}

        {/* History Tab */}
        {activeTab === "history" && (
          <div className="text-center py-8 text-gray-500">
            <Vote size={32} className="mx-auto mb-3 opacity-50" />
            <p className="text-sm">No voting history yet</p>
          </div>
        )}
      </div>

      <div className="text-xs text-gray-500 text-center pt-4 border-t border-[#2a2a35]">
        DAO governance voting with time-locks and quorum tracking
      </div>
    </div>
  );
}
