#!/usr/bin/env python3
"""
Chainlink Multi-Vector Vulnerability Scanner
Hunts for common vulnerability patterns across Chainlink repositories
"""

import os
import re
from pathlib import Path
from collections import defaultdict

class ChainlinkVulnScanner:
    def __init__(self, repo_path="/tmp/chainlink_repos"):
        self.repo_path = repo_path
        self.findings = defaultdict(list)
        
    def scan_all(self):
        """Execute all scans"""
        print("[*] Starting Chainlink Vulnerability Scan...")
        
        self.scan_oracle_vulnerabilities()
        self.scan_access_control_issues()
        self.scan_cryptographic_issues()
        self.scan_input_validation()
        self.scan_job_creation_vulns()
        self.scan_external_adapter_vulns()
        self.scan_permit_functions()
        self.scan_upgrade_vulnerabilities()
        
        self.report_findings()
    
    def scan_oracle_vulnerabilities(self):
        """Scan for oracle manipulation vulnerabilities"""
        print("[*] Scanning for oracle vulnerabilities...")
        patterns = [
            (r'function\s+\w*[Oo]racle\w*\([^)]*\)\s*(?:public|external).*?\{', 'Oracle function without access control'),
            (r'roundId.*?answer.*?updatedAt', 'Oracle data validation'),
            (r'staleness.*?check.*?if.*?block\.timestamp', 'Stale data handling'),
            (r'latestRoundData\(\)', 'Raw oracle call without staleness check'),
            (r'_getLatestPrice.*?require.*?price\s*>=\s*0', 'Price validation bypass'),
        ]
        
        self._scan_patterns('oracle', patterns, ['*.sol', '*.ts', '*.go'])
    
    def scan_access_control_issues(self):
        """Scan for access control vulnerabilities"""
        print("[*] Scanning for access control issues...")
        patterns = [
            (r'function\s+\w*\([^)]*\)\s*(?:public|external)[^{]*\{(?!.*onlyOwner|.*onlyRole|.*require)', 'Public function without access control'),
            (r'msg\.sender\s*==\s*owner', 'Direct owner check (should use modifier)'),
            (r'if\s*\(\s*msg\.sender\s*!=\s*[\w\.]+\s*\)\s*return', 'Ineffective access control'),
            (r'pause\(\).*?unpause\(\)', 'Pause/unpause without proper guards'),
            (r'transferOwnership.*?require\(.*?\)', 'Ownership transfer validation'),
        ]
        
        self._scan_patterns('access_control', patterns, ['*.sol', '*.go'])
    
    def scan_cryptographic_issues(self):
        """Scan for cryptographic weaknesses"""
        print("[*] Scanning for cryptographic issues...")
        patterns = [
            (r'sha256\s*\(\s*abi\.encodePacked', 'SHA256 with encodePacked (collision risk)'),
            (r'keccak256\s*\(\s*abi\.encodePacked.*?address.*?uint', 'Hash collision risk with address+uint'),
            (r'random.*?block\.timestamp.*?block\.number', 'Weak randomness sources'),
            (r'nonce.*?increment.*?no.*?check', 'Nonce management issue'),
            (r'signature.*?ecrecover.*?no.*?nonce', 'Signature replay vulnerability'),
            (r'recover.*?require.*?!=.*?address\(0\)', 'Signature validation'),
        ]
        
        self._scan_patterns('cryptographic', patterns, ['*.sol'])
    
    def scan_input_validation(self):
        """Scan for input validation issues"""
        print("[*] Scanning for input validation issues...")
        patterns = [
            (r'function\s+\w+\([^)]*\s+bytes\s+\w+[^)]*\)[^{]*\{(?!.*validate|.*require.*\.length)', 'Unvalidated bytes input'),
            (r'delegatecall.*?\(', 'Delegatecall usage'),
            (r'abi\.decode.*?require.*?length', 'Decoded data validation'),
            (r'transfer.*?amount[^;]*;(?!.*require.*?amount)', 'Transfer without amount validation'),
            (r'for\s*\([^)]*\)\s*{[^}]*delegatecall', 'Delegatecall in loop'),
        ]
        
        self._scan_patterns('input_validation', patterns, ['*.sol'])
    
    def scan_job_creation_vulns(self):
        """Scan for job creation vulnerabilities"""
        print("[*] Scanning for job creation vulnerabilities...")
        patterns = [
            (r'createJob.*?\{', 'Job creation without validation'),
            (r'addTask.*?ExternalAdapter', 'External adapter task without URI validation'),
            (r'parseResponse.*?abi\.decode.*?no.*?bounds', 'Unbounded response parsing'),
            (r'httpGet.*?url.*?!.*?validUrl', 'HTTP request without URL validation'),
            (r'setFulfillmentFunction.*?require.*?authorized', 'Fulfillment function permissions'),
        ]
        
        self._scan_patterns('job_creation', patterns, ['*.sol', '*.go', '*.ts'])
    
    def scan_external_adapter_vulns(self):
        """Scan for external adapter vulnerabilities"""
        print("[*] Scanning for external adapter vulnerabilities...")
        patterns = [
            (r'POST.*?url.*?no.*?validation', 'POST request without validation'),
            (r'parse.*?JSON.*?require.*?!=.*?nil', 'Unvalidated JSON response'),
            (r'adapter.*?authenticate.*?hardcoded', 'Hardcoded credentials'),
            (r'retry.*?attempt.*?no.*?backoff', 'Retry logic without backoff'),
            (r'timeout.*?set.*?<\s*1.*?second', 'Insufficient timeout'),
        ]
        
        self._scan_patterns('external_adapter', patterns, ['*.go', '*.ts', '*.py'])
    
    def scan_permit_functions(self):
        """Scan for EIP-2612 permit vulnerabilities"""
        print("[*] Scanning for permit() vulnerabilities...")
        patterns = [
            (r'function\s+permit\s*\([^)]*deadline[^)]*\)[^{]*\{(?!.*require.*?deadline|.*?deadline\s*>=)', 'Permit without deadline check'),
            (r'permit.*?ecrecover.*?owner\s*=\s*recovered', 'Permit owner validation'),
            (r'nonce.*?permit.*?++', 'Permit nonce handling'),
            (r'_permit.*?domainSeparator', 'Permit domain separator'),
        ]
        
        self._scan_patterns('permit', patterns, ['*.sol'])
    
    def scan_upgrade_vulnerabilities(self):
        """Scan for upgrade/proxy vulnerabilities"""
        print("[*] Scanning for upgrade vulnerabilities...")
        patterns = [
            (r'upgradeTo.*?\{(?!.*onlyAdmin|.*onlyOwner)', 'Upgrade without access control'),
            (r'_authorizeUpgrade.*?\{.*?onlyOwner', 'Upgrade authorization'),
            (r'initialize.*?\{(?!.*initializer)', 'Initializer without guard'),
            (r'delegatecall.*?implementation', 'Proxy delegatecall'),
            (r'selfdestruct.*?\{', 'Selfdestruct in upgradeable contract'),
        ]
        
        self._scan_patterns('upgrade', patterns, ['*.sol'])
    
    def _scan_patterns(self, category, patterns, file_types):
        """Generic pattern scanner"""
        for root, dirs, files in os.walk(self.repo_path):
            # Skip node_modules, build dirs
            dirs[:] = [d for d in dirs if d not in ['node_modules', 'build', 'dist', '.git', '__pycache__']]
            
            for file in files:
                # Match file types
                if not any(file.endswith(ft.replace('*', '')) for ft in file_types):
                    continue
                
                filepath = os.path.join(root, file)
                try:
                    with open(filepath, 'r', encoding='utf-8', errors='ignore') as f:
                        content = f.read()
                        line_num = 1
                        
                        for pattern, description in patterns:
                            matches = re.finditer(pattern, content, re.MULTILINE | re.DOTALL)
                            for match in matches:
                                # Count lines to get approximate line number
                                line_num = content[:match.start()].count('\n') + 1
                                
                                self.findings[category].append({
                                    'file': filepath.replace(self.repo_path, ''),
                                    'line': line_num,
                                    'pattern': description,
                                    'severity': self._estimate_severity(category, description)
                                })
                except Exception as e:
                    pass
    
    def _estimate_severity(self, category, description):
        """Estimate CVSS severity"""
        critical_keywords = ['delegatecall', 'selfdestruct', 'ecrecover', 'signature', 'authorization', 'onlyOwner']
        high_keywords = ['validation', 'overflow', 'underflow', 'require', 'transfer', 'pausable']
        
        if any(kw in description.lower() for kw in critical_keywords):
            return 'CRITICAL (9.0-10.0)'
        elif any(kw in description.lower() for kw in high_keywords):
            return 'HIGH (7.0-8.9)'
        else:
            return 'MEDIUM (4.0-6.9)'
    
    def report_findings(self):
        """Generate report"""
        print("\n" + "="*80)
        print("CHAINLINK VULNERABILITY SCAN REPORT")
        print("="*80 + "\n")
        
        total = 0
        for category in sorted(self.findings.keys()):
            findings = self.findings[category]
            total += len(findings)
            
            print(f"\n[{category.upper()}] - {len(findings)} potential issues found\n")
            
            for finding in sorted(findings, key=lambda x: x['severity'], reverse=True)[:5]:  # Top 5 per category
                print(f"  Severity: {finding['severity']}")
                print(f"  File: {finding['file']}")
                print(f"  Line: {finding['line']}")
                print(f"  Pattern: {finding['pattern']}")
                print()
        
        print("="*80)
        print(f"TOTAL POTENTIAL FINDINGS: {total}")
        print("="*80)
        
        # Save detailed report
        with open('/tmp/chainlink_vuln_scan.txt', 'w') as f:
            f.write("CHAINLINK VULNERABILITY SCAN REPORT\n")
            f.write(f"Date: {os.popen('date').read()}\n")
            f.write("="*80 + "\n\n")
            
            for category in sorted(self.findings.keys()):
                f.write(f"\n[{category.upper()}]\n")
                f.write("-"*80 + "\n")
                for finding in self.findings[category]:
                    f.write(f"File: {finding['file']}\n")
                    f.write(f"Line: {finding['line']}\n")
                    f.write(f"Severity: {finding['severity']}\n")
                    f.write(f"Pattern: {finding['pattern']}\n\n")
        
        print("\nDetailed report saved to: /tmp/chainlink_vuln_scan.txt")

if __name__ == "__main__":
    scanner = ChainlinkVulnScanner()
    scanner.scan_all()
