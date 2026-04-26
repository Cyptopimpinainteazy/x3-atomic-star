import React, { useState } from 'react';
import { Download, CheckCircle, AlertCircle, Clock, Settings, ChevronDown } from 'lucide-react';
import clsx from 'clsx';

interface ChangelogEntry {
  version: string;
  date: string;
  highlights: string[];
  improvements: string[];
  fixes: string[];
}

interface UpdateInfo {
  available: boolean;
  currentVersion: string;
  latestVersion?: string;
  releaseDate?: string;
  downloadSize?: string;
  changelog?: ChangelogEntry;
}

const MOCK_CHANGELOG: ChangelogEntry[] = [
  {
    version: '2.1.0',
    date: 'Dec 20, 2024',
    highlights: [
      'Validator leaderboard with GPU scoring',
      'Social tipping system (X3 micropayments)',
      'veX3 vote-escrow tokenomics UI',
      'Pool analytics dashboard',
    ],
    improvements: [
      'Whale tracker alerts for large transfers (>$100k)',
      'Creator monetization with subscription UI',
      'Proof-of-human verification integration',
      'NFT profile picture support',
    ],
    fixes: [
      'Fixed wallet panel lag with 1000+ transactions',
      'Improved validator globe performance on low-end devices',
      'Fixed dark mode toggle persistence',
    ],
  },
  {
    version: '2.0.5',
    date: 'Dec 15, 2024',
    highlights: [
      'Advanced DEX orders (limit orders, TWAP, stop-loss)',
      'Dashboard CSV export with date filtering',
      'Hardware wallet support (Ledger + Trezor)',
    ],
    improvements: [
      'Transaction history auto-labeling',
      'Watch-only wallet mode',
      'QR code scanner for address input',
      'Encrypted local backup export',
    ],
    fixes: [
      'Fixed infinite scroll in transaction history',
      'Resolved WebSocket reconnection issues',
    ],
  },
  {
    version: '2.0.0',
    date: 'Dec 1, 2024',
    highlights: [
      'X3 Desktop v2 launch',
      'Multi-panel architecture',
      'Real-time validator monitoring',
      'Complete wallet overhaul',
    ],
    improvements: [],
    fixes: [],
  },
];

const DesktopUpdatesPanel: React.FC = () => {
  const [updateInfo] = useState<UpdateInfo>({
    available: true,
    currentVersion: '2.0.5',
    latestVersion: '2.1.0',
    releaseDate: 'Dec 20, 2024',
    downloadSize: '182 MB',
    changelog: MOCK_CHANGELOG[0],
  });

  const [expandedVersion, setExpandedVersion] = useState<string | null>(updateInfo.latestVersion || null);
  const [downloading, setDownloading] = useState(false);
  const [downloadProgress, setDownloadProgress] = useState(0);

  const handleUpdate = () => {
    setDownloading(true);
    // Simulate download progress
    let progress = 0;
    const interval = setInterval(() => {
      progress += Math.random() * 25;
      if (progress > 100) {
        progress = 100;
        clearInterval(interval);
      }
      setDownloadProgress(Math.min(progress, 100));
    }, 400);
  };

  return (
    <div className="h-full flex flex-col bg-[#0a0a0f] text-white overflow-auto">
      {/* Header */}
      <div className="flex items-center justify-between px-5 py-4 border-b border-[#1a1a1a]">
        <div className="flex items-center gap-3">
          <Settings size={18} className="text-blue-400" />
          <h1 className="text-lg font-bold">Desktop Updates</h1>
        </div>
        <div className="flex items-center gap-2 text-xs font-mono text-gray-500">
          v{updateInfo.currentVersion}
        </div>
      </div>

      {/* Update Available Banner */}
      {updateInfo.available && (
        <div className="bg-gradient-to-r from-blue-500/20 to-purple-500/20 border border-blue-500/40 m-5 rounded-xl p-5">
          <div className="flex items-start gap-4">
            <div className="bg-blue-500/20 rounded-lg p-3 flex-shrink-0">
              <Download size={20} className="text-blue-400" />
            </div>
            <div className="flex-1">
              <h3 className="font-bold text-white mb-1 flex items-center gap-2">
                Update Available
                <span className="text-xs bg-blue-500/40 text-blue-300 px-2 py-0.5 rounded">NEW</span>
              </h3>
              <p className="text-sm text-gray-400 mb-3">
                Version <span className="text-blue-400 font-semibold">{updateInfo.latestVersion}</span> is ready to install ({updateInfo.downloadSize})
              </p>
              <div className="text-xs text-gray-500 mb-4">
                Released: {updateInfo.releaseDate}
              </div>

              {!downloading ? (
                <button
                  onClick={handleUpdate}
                  className="flex items-center gap-2 bg-gradient-to-r from-blue-500 to-blue-600 hover:from-blue-400 hover:to-blue-500 text-white px-4 py-2 rounded-lg font-semibold text-sm transition-all shadow-lg shadow-blue-500/20"
                >
                  <Download size={14} /> Update Now
                </button>
              ) : (
                <div className="space-y-2">
                  <div className="flex justify-between text-xs mb-1">
                    <span>Downloading update...</span>
                    <span className="text-blue-400">{Math.round(downloadProgress)}%</span>
                  </div>
                  <div className="w-full bg-[#0a0a0f] rounded-full h-2 border border-[#1a1a1a] overflow-hidden">
                    <div
                      className="bg-gradient-to-r from-blue-500 to-blue-600 h-full transition-all duration-300"
                      style={{ width: `${downloadProgress}%` }}
                    />
                  </div>
                </div>
              )}
            </div>
          </div>
        </div>
      )}

      {/* Changelog */}
      <div className="px-5 py-4">
        <h2 className="text-sm font-semibold text-gray-400 mb-3">Release History</h2>
        
        <div className="space-y-3">
          {MOCK_CHANGELOG.map((entry) => (
            <div
              key={entry.version}
              className="bg-[#111111] border border-[#1a1a1a] rounded-lg overflow-hidden hover:border-[#2a2a2a] transition-colors"
            >
              <button
                onClick={() => setExpandedVersion(expandedVersion === entry.version ? null : entry.version)}
                className="w-full flex items-center justify-between p-4 hover:bg-[#0f0f14] transition-colors"
              >
                <div className="flex items-center gap-3">
                  {entry.version === updateInfo.latestVersion ? (
                    <div className="bg-blue-500/20 rounded-lg p-2">
                      <CheckCircle size={16} className="text-blue-400" />
                    </div>
                  ) : (
                    <div className="bg-[#0a0a0f] rounded-lg p-2">
                      <Clock size={16} className="text-gray-500" />
                    </div>
                  )}
                  <div className="text-left">
                    <div className="font-semibold text-white flex items-center gap-2">
                      v{entry.version}
                      {entry.version === updateInfo.latestVersion && (
                        <span className="text-xs bg-blue-500/40 text-blue-300 px-2 py-0.5 rounded">Latest</span>
                      )}
                    </div>
                    <div className="text-xs text-gray-500">{entry.date}</div>
                  </div>
                </div>
                <ChevronDown
                  size={16}
                  className={clsx(
                    'text-gray-500 transition-transform',
                    expandedVersion === entry.version && 'rotate-180'
                  )}
                />
              </button>

              {/* Expanded Content */}
              {expandedVersion === entry.version && (
                <div className="border-t border-[#1a1a1a] bg-[#0a0a0f] p-4 space-y-4">
                  {entry.highlights.length > 0 && (
                    <div>
                      <h4 className="text-xs font-semibold text-yellow-400 mb-2 flex items-center gap-1">
                        ✨ Highlights
                      </h4>
                      <ul className="space-y-1 text-xs text-gray-300">
                        {entry.highlights.map((item, idx) => (
                          <li key={idx} className="flex gap-2">
                            <span className="text-blue-400">•</span> {item}
                          </li>
                        ))}
                      </ul>
                    </div>
                  )}

                  {entry.improvements.length > 0 && (
                    <div>
                      <h4 className="text-xs font-semibold text-green-400 mb-2 flex items-center gap-1">
                        📈 Improvements
                      </h4>
                      <ul className="space-y-1 text-xs text-gray-300">
                        {entry.improvements.map((item, idx) => (
                          <li key={idx} className="flex gap-2">
                            <span className="text-green-400">•</span> {item}
                          </li>
                        ))}
                      </ul>
                    </div>
                  )}

                  {entry.fixes.length > 0 && (
                    <div>
                      <h4 className="text-xs font-semibold text-red-400 mb-2 flex items-center gap-1">
                        🔧 Fixes
                      </h4>
                      <ul className="space-y-1 text-xs text-gray-300">
                        {entry.fixes.map((item, idx) => (
                          <li key={idx} className="flex gap-2">
                            <span className="text-red-400">•</span> {item}
                          </li>
                        ))}
                      </ul>
                    </div>
                  )}
                </div>
              )}
            </div>
          ))}
        </div>
      </div>

      {/* Settings */}
      <div className="mt-auto px-5 py-4 border-t border-[#1a1a1a]">
        <div className="bg-[#111111] rounded-lg p-4 space-y-3">
          <div className="flex items-center justify-between">
            <span className="text-sm text-gray-400">Auto-update</span>
            <button className="w-12 h-6 bg-blue-500 rounded-full transition-colors" />
          </div>
          <div className="flex items-center justify-between">
            <span className="text-sm text-gray-400">Notify on release</span>
            <button className="w-12 h-6 bg-blue-500 rounded-full transition-colors" />
          </div>
          <div className="text-xs text-gray-600 pt-2 border-t border-[#1a1a1a]">
            Last checked: 2 minutes ago
          </div>
        </div>
      </div>
    </div>
  );
};

export default DesktopUpdatesPanel;

