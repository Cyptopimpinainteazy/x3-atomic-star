import { useState } from "react";
import { ShieldAlert, Users, BrainCircuit, Activity, CheckCircle, Clock, ArrowRight, XCircle } from "lucide-react";
import clsx from "clsx";

interface TaskProposal {
  id: string;
  source: string;
  type: "chain_mutation" | "autonomous_action";
  title: string;
  description: string;
  status: "pending_crm" | "approved" | "rejected" | "auto_executed";
  votesYes: number;
  votesNo: number;
  requiredVotes: number;
  timeRemaining?: string;
  riskLevel: "Critical" | "High" | "Medium" | "Low";
  impactScore: number;
}

const MOCK_TASKS: TaskProposal[] = [
  {
    id: "task-001",
    source: "Arbitrage Scanner Agent",
    type: "chain_mutation",
    title: "Execute Cross-Chain Liquidity Shift",
    description: "Detected systemic arb route via DEX-A & SUSHI. Requires moving 5M X3 tokens from cold to hot bridge.",
    status: "pending_crm",
    votesYes: 3,
    votesNo: 1,
    requiredVotes: 5,
    timeRemaining: "14m 20s",
    riskLevel: "Critical",
    impactScore: 94,
  },
  {
    id: "task-002",
    source: "Social Marketing Agent",
    type: "autonomous_action",
    title: "Join SEO Trading Group & Post Bio",
    description: "Identified high-conversion social group on LinkedIn/Discord. Bio matches targets.",
    status: "auto_executed",
    votesYes: 0,
    votesNo: 0,
    requiredVotes: 0,
    riskLevel: "Low",
    impactScore: 45,
  },
  {
    id: "task-003",
    source: "Protocol Optimizer Swarm",
    type: "chain_mutation",
    title: "Adjust Global Gas Fees -0.5%",
    description: "Network congestion down 18%. Swarm proposes reducing base fee to incentivize TPS.",
    status: "pending_crm",
    votesYes: 4,
    votesNo: 0,
    requiredVotes: 5,
    timeRemaining: "4h 12m",
    riskLevel: "High",
    impactScore: 82,
  },
  {
    id: "task-004",
    source: "Content Generator Agent",
    type: "autonomous_action",
    title: "Publish Weekend Market Recap Blog",
    description: "Compiled weekly trading data into a 1500 word post for SEO optimization.",
    status: "auto_executed",
    votesYes: 0,
    votesNo: 0,
    requiredVotes: 0,
    riskLevel: "Low",
    impactScore: 61,
  }
];

export default function CrmGovernancePanel() {
  const [tasks, setTasks] = useState<TaskProposal[]>(MOCK_TASKS);

  const handleVote = (taskId: string, voteType: "yes" | "no") => {
    setTasks((prev) =>
      prev.map((task) => {
        if (task.id === taskId && task.status === "pending_crm") {
          const newTask = { ...task };
          if (voteType === "yes") newTask.votesYes += 1;
          else newTask.votesNo += 1;

          // Check required threshold
          if (newTask.votesYes >= newTask.requiredVotes) {
            newTask.status = "approved";
          } else if (newTask.votesNo >= Math.ceil(newTask.requiredVotes / 2)) {
            // Fails early if impossible to reach majority
            newTask.status = "rejected";
          }
          return newTask;
        }
        return task;
      })
    );
  };

  const getRiskColor = (risk: string) => {
    switch (risk) {
      case "Critical": return "text-red-500 bg-red-500/10 border-red-500/20";
      case "High": return "text-orange-500 bg-orange-500/10 border-orange-500/20";
      case "Medium": return "text-yellow-500 bg-yellow-500/10 border-yellow-500/20";
      case "Low": return "text-green-500 bg-green-500/10 border-green-500/20";
      default: return "text-gray-400 bg-gray-500/10 border-gray-500/20";
    }
  };

  return (
    <div className="w-full h-full bg-[#0a0a0f] text-white p-6 flex flex-col font-sans">
      <div className="flex justify-between items-center mb-6 border-b border-[#2a2a35] pb-4">
        <div>
          <h2 className="text-2xl font-bold flex items-center gap-2 bg-gradient-to-r from-purple-400 to-cyan-400 bg-clip-text text-transparent">
            <Users size={24} className="text-purple-400" /> CRM Governance Window
          </h2>
          <p className="text-sm text-gray-400 mt-1">
            Human-in-the-Loop Override & Authorizations
          </p>
        </div>
        <div className="bg-[#15151b] px-4 py-2 rounded-lg border border-[#2a2a35] flex items-center gap-3 shadow-lg">
          <Activity className="text-cyan-400 animate-pulse" size={16} />
          <span className="text-sm font-semibold">Live Swarm Connection</span>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto space-y-4 pr-2 custom-scrollbar">
        {tasks.map((task) => (
          <div
            key={task.id}
            className={clsx(
              "rounded-xl border p-5 backdrop-blur-md transition-all duration-300",
              task.type === "chain_mutation" 
                ? "bg-gradient-to-br from-[#1c1525] to-[#15151b] border-purple-500/30 hover:border-purple-500/60 shadow-[0_0_15px_rgba(168,85,247,0.05)]"
                : "bg-gradient-to-br from-[#101920] to-[#15151b] border-cyan-500/30 hover:border-cyan-500/60"
            )}
          >
            <div className="flex justify-between items-start mb-3">
              <div className="flex items-center gap-3">
                {task.type === "chain_mutation" ? (
                  <ShieldAlert className="text-purple-400" size={20} />
                ) : (
                  <BrainCircuit className="text-cyan-400" size={20} />
                )}
                <div>
                  <h3 className="font-bold text-lg text-gray-100">{task.title}</h3>
                  <div className="text-xs font-mono text-gray-500 uppercase tracking-widest mt-1">
                    Source: <span className="text-gray-300">{task.source}</span>
                  </div>
                </div>
              </div>
              
              <div className="flex flex-col items-end gap-2">
                <span className={clsx(
                  "px-3 py-1 rounded-full text-xs font-bold border",
                  getRiskColor(task.riskLevel)
                )}>
                  {task.riskLevel} Risk
                </span>
                {task.status === "pending_crm" && (
                  <span className="flex items-center gap-1 text-xs text-orange-400 bg-orange-400/10 px-2 py-1 rounded">
                    <Clock size={12} /> {task.timeRemaining}
                  </span>
                )}
              </div>
            </div>

            <p className="text-sm text-gray-400 leading-relaxed max-w-3xl mb-4">
              {task.description}
            </p>

            {/* Voting Component for Chain Mutations */}
            {task.type === "chain_mutation" ? (
              <div className="bg-[#0f0f13] border border-[#2a2a35] rounded-lg p-4 mt-2">
                <div className="flex justify-between items-center mb-3">
                  <div className="text-sm font-semibold text-gray-300">
                    Human Authorization Matrix
                  </div>
                  <div className="text-xs text-gray-500">
                    {task.votesYes} / {task.requiredVotes} Votes Reached
                  </div>
                </div>

                <div className="w-full bg-[#15151b] rounded-full h-2.5 mb-4 overflow-hidden border border-[#2a2a35]">
                  <div
                    className={clsx(
                      "h-full transition-all duration-500 ease-out",
                      task.status === "approved" ? "bg-green-500" : "bg-purple-500 shadow-[0_0_10px_rgba(168,85,247,0.5)]"
                    )}
                    style={{ width: `${Math.min((task.votesYes / task.requiredVotes) * 100, 100)}%` }}
                  ></div>
                </div>

                {task.status === "pending_crm" ? (
                  <div className="flex gap-3">
                    <button
                      onClick={() => handleVote(task.id, "yes")}
                      className="flex-1 flex justify-center items-center gap-2 bg-gradient-to-r from-green-600/20 to-green-500/10 hover:from-green-500/30 hover:to-green-400/20 border border-green-500/50 text-green-400 py-2 rounded-md font-bold text-sm transition-all"
                    >
                      <CheckCircle size={16} /> Approve
                    </button>
                    <button
                      onClick={() => handleVote(task.id, "no")}
                      className="flex-1 flex justify-center items-center gap-2 bg-gradient-to-r from-red-600/20 to-red-500/10 hover:from-red-500/30 hover:to-red-400/20 border border-red-500/50 text-red-400 py-2 rounded-md font-bold text-sm transition-all"
                    >
                      <XCircle size={16} /> Reject
                    </button>
                  </div>
                ) : (
                  <div className="flex justify-center items-center py-2 bg-[#1c2a20] border border-green-900 rounded-md text-green-400 text-sm font-bold gap-2">
                    <CheckCircle size={16} /> Execution Authorized. Sending to Agent Swarm...
                  </div>
                )}
              </div>
            ) : (
              /* Autonomous Action Read-Only Component */
              <div className="bg-[#0f1115] border border-cyan-500/20 rounded-lg p-3 mt-2 flex items-center justify-between">
                <div className="flex items-center gap-2 text-sm text-cyan-400 font-semibold">
                  <BrainCircuit size={16} /> Swarm Court Authorized (No Human CRM Required)
                </div>
                <div className="text-xs text-gray-500 flex items-center gap-1">
                  Task Impact: <span className="text-white font-mono">{task.impactScore}</span> <ArrowRight size={12} /> Executed
                </div>
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}
