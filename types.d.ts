type Statistics = {
   cpuUsage: number
   ramUsage: number
   storageData: number
}

type StaticData = {
   totalStorage: number
   cpuModel: string
   totalMemoryGB: number
}

type UnsubscribeFunction = () => void

type EventPayloadMapping = {
   statistics: Statistics
   getStaticData: StaticData
   "get-screen-time-data": ScreenTimeData | null
}

interface AppUsage {
   name: string
   window_title: string
   seconds: number
   category?: string
}

interface DayStats {
   date: string
   total_seconds: number
   apps: Record<string, number>
}

interface ScreenTimeData {
   today_seconds: number
   today_date: string
   weekly_stats: DayStats[]
   app_usages: AppUsage[]
}

interface Window {
   electron: {
      subscribeStatistics: (callback: (statistics: Statistics) => void) => UnsubscribeFunction
      getStaticData: () => Promise<StaticData>
      getScreenTimeData: () => Promise<ScreenTimeData | null>
   }
}