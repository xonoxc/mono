import { spawn } from "child_process";
import { app } from "electron";
import path from "path";
import os from "os";

let trackerProcess: ReturnType<typeof spawn> | null = null;
let pollInterval: NodeJS.Timeout | null = null;

export function getStaticData() {
    return {
        totalStorage: 500,
        cpuModel: os.cpus()[0]?.model || "Unknown",
        totalMemoryGB: Math.round(os.totalmem() / (1024 * 1024 * 1024)),
    };
}

export function pollResources(mainWindow: Electron.BrowserWindow) {
    const sendStats = () => {
        const cpu = os.cpus();
        let totalIdle = 0;
        let totalTick = 0;

        cpu.forEach((c) => {
            for (const type in c.times) {
                totalTick += c.times[type as keyof typeof c.times];
            }
            totalIdle += c.times.idle;
        });

        const usage = 100 - Math.round((totalIdle / totalTick) * 100);
        const totalMem = os.totalmem();
        const freeMem = os.freemem();
        const usedMem = totalMem - freeMem;
        const memPercent = Math.round((usedMem / totalMem) * 100);

        mainWindow.webContents.send("statistics", {
            cpuUsage: usage,
            ramUsage: memPercent,
            storageData: 45,
        });
    };

    pollInterval = setInterval(sendStats, 1000);
    sendStats();
}

export function startTracker() {
    const trackerPath = app.isPackaged
        ? path.join(process.resourcesPath, "tracker-bin")
        : path.join(app.getAppPath(), "tracker-bin");

    try {
        trackerProcess = spawn(trackerPath, ["watch"], {
            detached: true,
            stdio: "ignore",
        });

        trackerProcess.unref();
        console.log("[Tracker] Started background process");
    } catch (e) {
        console.error("[Tracker] Failed to start:", e);
    }
}

export function getTrackerData(): Promise<ScreenTimeData> {
    return new Promise((resolve, reject) => {
        const trackerPath = app.isPackaged
            ? path.join(process.resourcesPath, "tracker-bin")
            : path.join(app.getAppPath(), "tracker-bin");

        const proc = spawn(trackerPath, ["get-data"], { shell: true });

        let stdout = "";
        let stderr = "";

        proc.stdout?.on("data", (data) => {
            stdout += data.toString();
        });

        proc.stderr?.on("data", (data) => {
            stderr += data.toString();
        });

        proc.on("close", (code) => {
            if (code === 0 && stdout) {
                try {
                    const data = JSON.parse(stdout.trim());
                    resolve(data);
                } catch (e) {
                    reject(new Error(`Failed to parse tracker data: ${stdout}`));
                }
            } else {
                reject(new Error(`Tracker exited with code ${code}: ${stderr}`));
            }
        });

        proc.on("error", (err) => {
            reject(err);
        });
    });
}

export interface ScreenTimeData {
    today_seconds: number;
    today_date: string;
    weekly_stats: DayStats[];
    app_usages: AppUsage[];
}

export interface DayStats {
    date: string;
    total_seconds: number;
    apps: Record<string, number>;
}

export interface AppUsage {
    name: string;
    window_title: string;
    seconds: number;
}

export function stopTracker() {
    if (trackerProcess) {
        trackerProcess.kill();
        trackerProcess = null;
    }
    if (pollInterval) {
        clearInterval(pollInterval);
        pollInterval = null;
    }
}