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
        trackerProcess = spawn(trackerPath, [], {
            detached: true,
            stdio: "ignore",
        });

        trackerProcess.unref();
        console.log("[Tracker] Started background process");
    } catch (e) {
        console.error("[Tracker] Failed to start:", e);
    }
}

export async function getTrackerData(): Promise<ScreenTimeData | null> {
    try {
        const todayRes = await fetch("http://127.0.0.1:9746/today-usage");
        if (!todayRes.ok) return null;
        const todayData = await todayRes.json();

        const weeklyRes = await fetch("http://127.0.0.1:9746/weekly-usage");
        if (!weeklyRes.ok) return null;
        const weeklyData = await weeklyRes.json();

        return {
            today_seconds: todayData.data.total_seconds,
            today_date: todayData.data.date,
            weekly_stats: weeklyData.data.days.map((d: any) => ({
                date: d.date,
                total_seconds: d.total_seconds,
                apps: d.top_apps.reduce((acc: any, app: any) => {
                    acc[app.app_name] = app.seconds;
                    return acc;
                }, {})
            })),
            app_usages: todayData.data.app_breakdown.map((app: any) => ({
                name: app.app_name,
                window_title: app.app_name, // fallback
                seconds: app.seconds,
                category: app.category,
            }))
        };
    } catch (e) {
        console.error("Failed to fetch from tracking daemon", e);
        return null;
    }
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