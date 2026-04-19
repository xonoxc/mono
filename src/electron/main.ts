import { app, BrowserWindow, ipcMain } from "electron"
import { ipcMainHandle, isDev } from "./util.js"
import { getPreloadPath, getUIPath, getIconPath } from "./pathResolver.js"
import { getStaticData, pollResources, startTracker, getTrackerData, stopTracker } from "./test.js"
import dotenv from "dotenv"

dotenv.config()

const PORT = process.env.PORT || "5173"

app.on("ready", () => {
   const mainWindow = new BrowserWindow({
      webPreferences: {
         preload: getPreloadPath(),
      },
      icon: getIconPath(),
      autoHideMenuBar: true,
   })

   if (isDev()) mainWindow.loadURL(`http://localhost:${PORT}`)
   else mainWindow.loadFile(getUIPath())

   pollResources(mainWindow)

   startTracker()

   ipcMainHandle("getStaticData", () => {
      return getStaticData()
   })

   ipcMain.handle("get-screen-time-data", async () => {
      try {
         return await getTrackerData()
      } catch (e) {
         console.error("Failed to get tracker data:", e)
         return null
      }
   })
})

app.on("will-quit", () => {
   stopTracker()
})

