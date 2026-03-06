// src/binaryManager.ts
import * as vscode from 'vscode';
import * as fs from 'fs';
import * as path from 'path';
import * as https from 'https';
import * as http from 'http';

const GITHUB_REPO = 'codeyousef/seenlang';
const BINARY_NAME = process.platform === 'win32' ? 'seen.exe' : 'seen';

interface GitHubRelease {
    tag_name: string;
    assets: Array<{
        name: string;
        browser_download_url: string;
    }>;
}

export class BinaryManager {
    private context: vscode.ExtensionContext;
    private binaryPath: string;
    private versionPath: string;

    constructor(context: vscode.ExtensionContext) {
        this.context = context;
        const storageDir = context.globalStorageUri.fsPath;
        this.binaryPath = path.join(storageDir, BINARY_NAME);
        this.versionPath = path.join(storageDir, 'version.txt');
    }

    /**
     * Ensures a working Seen compiler binary is available.
     * Downloads from GitHub releases if not present.
     * @returns Path to the Seen compiler binary
     */
    async ensureBinary(): Promise<string> {
        // Ensure storage directory exists
        await this.ensureStorageDir();

        // Check if binary exists and is executable
        if (await this.binaryExists()) {
            console.log('Seen compiler binary found at:', this.binaryPath);
            return this.binaryPath;
        }

        // Download with progress
        await this.downloadBinary();
        return this.binaryPath;
    }

    /**
     * Force update the compiler binary to the latest version
     */
    async forceUpdate(): Promise<void> {
        // Remove existing binary if present
        try {
            await fs.promises.unlink(this.binaryPath);
        } catch {
            // File may not exist, that's fine
        }

        await this.downloadBinary();
    }

    /**
     * Get the path to the binary (may not exist)
     */
    getBinaryPath(): string {
        return this.binaryPath;
    }

    /**
     * Check if an update is available
     */
    async checkForUpdate(): Promise<boolean> {
        try {
            const currentVersion = await this.getCurrentVersion();
            if (!currentVersion) {
                return true; // No version means we need to download
            }

            const latestRelease = await this.getLatestRelease();
            return latestRelease.tag_name !== currentVersion;
        } catch {
            return false; // Error checking, assume no update needed
        }
    }

    private async ensureStorageDir(): Promise<void> {
        const storageDir = this.context.globalStorageUri.fsPath;
        try {
            await fs.promises.mkdir(storageDir, { recursive: true });
        } catch (error) {
            // Directory may already exist
        }
    }

    private async binaryExists(): Promise<boolean> {
        try {
            await fs.promises.access(this.binaryPath, fs.constants.X_OK);
            return true;
        } catch {
            return false;
        }
    }

    private async getCurrentVersion(): Promise<string | null> {
        try {
            const version = await fs.promises.readFile(this.versionPath, 'utf-8');
            return version.trim();
        } catch {
            return null;
        }
    }

    private async saveCurrentVersion(version: string): Promise<void> {
        await fs.promises.writeFile(this.versionPath, version, 'utf-8');
    }

    private getPlatformId(): string {
        const platform = process.platform;
        const arch = process.arch;

        // Map Node.js platform/arch to our binary naming convention
        if (platform === 'linux' && arch === 'x64') {
            return 'linux-x64';
        }
        if (platform === 'linux' && arch === 'arm64') {
            return 'linux-arm64';
        }
        if (platform === 'darwin' && arch === 'x64') {
            return 'darwin-x64';
        }
        if (platform === 'darwin' && arch === 'arm64') {
            return 'darwin-arm64';
        }
        if (platform === 'win32' && arch === 'x64') {
            return 'win32-x64';
        }

        throw new Error(
            `Seen compiler auto-download is not available for your platform (${platform}-${arch}). ` +
            `Please install the compiler manually and set "seen.compiler.path" in settings.`
        );
    }

    private async getLatestRelease(): Promise<GitHubRelease> {
        return new Promise((resolve, reject) => {
            const options = {
                hostname: 'api.github.com',
                path: `/repos/${GITHUB_REPO}/releases/latest`,
                headers: {
                    'User-Agent': 'Seen-VSCode-Extension',
                    'Accept': 'application/vnd.github.v3+json'
                }
            };

            const req = https.get(options, (res) => {
                if (res.statusCode === 404) {
                    reject(new Error('No releases found. The Seen compiler may not have been released yet.'));
                    return;
                }

                if (res.statusCode !== 200) {
                    reject(new Error(`GitHub API returned status ${res.statusCode}`));
                    return;
                }

                let data = '';
                res.on('data', chunk => data += chunk);
                res.on('end', () => {
                    try {
                        const release = JSON.parse(data) as GitHubRelease;
                        resolve(release);
                    } catch (error) {
                        reject(new Error('Failed to parse GitHub release data'));
                    }
                });
            });

            req.on('error', reject);
            req.setTimeout(30000, () => {
                req.destroy();
                reject(new Error('Request to GitHub timed out'));
            });
        });
    }

    private async downloadBinary(): Promise<void> {
        const platformId = this.getPlatformId();

        await vscode.window.withProgress({
            location: vscode.ProgressLocation.Notification,
            title: "Downloading Seen compiler...",
            cancellable: false
        }, async (progress) => {
            try {
                progress.report({ message: 'Fetching release info...' });
                const release = await this.getLatestRelease();

                // Find the asset for our platform
                const assetName = `seen-${platformId}`;
                const asset = release.assets.find(a => a.name === assetName || a.name === `${assetName}.exe`);

                if (!asset) {
                    throw new Error(
                        `No binary found for ${platformId} in release ${release.tag_name}. ` +
                        `Available assets: ${release.assets.map(a => a.name).join(', ')}`
                    );
                }

                progress.report({ message: `Downloading ${release.tag_name}...` });
                await this.downloadFile(asset.browser_download_url, this.binaryPath, progress);

                progress.report({ message: 'Setting permissions...' });
                await this.makeExecutable();

                // Save version info
                await this.saveCurrentVersion(release.tag_name);

                progress.report({ message: 'Done!' });
            } catch (error) {
                // Clean up partial download on error
                try {
                    await fs.promises.unlink(this.binaryPath);
                } catch {
                    // Ignore cleanup errors
                }
                throw error;
            }
        });

        vscode.window.showInformationMessage('Seen compiler downloaded successfully!');
    }

    private async downloadFile(
        url: string,
        destPath: string,
        progress: vscode.Progress<{ message?: string; increment?: number }>
    ): Promise<void> {
        return new Promise((resolve, reject) => {
            const downloadWithRedirect = (downloadUrl: string, redirectCount: number = 0) => {
                if (redirectCount > 5) {
                    reject(new Error('Too many redirects'));
                    return;
                }

                const parsedUrl = new URL(downloadUrl);
                const protocol = parsedUrl.protocol === 'https:' ? https : http;

                const options = {
                    hostname: parsedUrl.hostname,
                    path: parsedUrl.pathname + parsedUrl.search,
                    headers: {
                        'User-Agent': 'Seen-VSCode-Extension'
                    }
                };

                const req = protocol.get(options, (res) => {
                    // Handle redirects
                    if (res.statusCode === 301 || res.statusCode === 302 || res.statusCode === 307) {
                        const location = res.headers.location;
                        if (location) {
                            downloadWithRedirect(location, redirectCount + 1);
                            return;
                        }
                    }

                    if (res.statusCode !== 200) {
                        reject(new Error(`Download failed with status ${res.statusCode}`));
                        return;
                    }

                    const totalSize = parseInt(res.headers['content-length'] || '0', 10);
                    let downloadedSize = 0;

                    const file = fs.createWriteStream(destPath);

                    res.on('data', (chunk) => {
                        downloadedSize += chunk.length;
                        if (totalSize > 0) {
                            const percent = Math.round((downloadedSize / totalSize) * 100);
                            progress.report({ message: `Downloading... ${percent}%` });
                        }
                    });

                    res.pipe(file);

                    file.on('finish', () => {
                        file.close();
                        resolve();
                    });

                    file.on('error', (err) => {
                        fs.unlink(destPath, () => {}); // Clean up
                        reject(err);
                    });
                });

                req.on('error', reject);
                req.setTimeout(120000, () => {
                    req.destroy();
                    reject(new Error('Download timed out'));
                });
            };

            downloadWithRedirect(url);
        });
    }

    private async makeExecutable(): Promise<void> {
        if (process.platform !== 'win32') {
            await fs.promises.chmod(this.binaryPath, 0o755);
        }
    }
}
