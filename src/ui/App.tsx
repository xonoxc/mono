import { useState, useEffect } from "react"
import {
   ChevronLeft,
   ChevronRight,
   ChevronDown,
   Focus,
   Timer,
   MoreHorizontal,
   Monitor,
   Youtube,
   Globe,
   Terminal,
} from "lucide-react"
import "./App.css"

const getAppIcon = (icon: string, category?: string) => {
   const iconProps = { size: 24 }
   const iconName = icon.toLowerCase()
   if (iconName.includes("youtube")) return <Youtube {...iconProps} color="#FF0000" />
   if (iconName.includes("chrome") || iconName.includes("firefox"))
      return <Globe {...iconProps} color="#4285F4" />
   if (
      iconName.includes("terminal") ||
      iconName.includes("alacritty") ||
      iconName.includes("konsole")
   )
      return <Terminal {...iconProps} color="#22c55e" />
   if (iconName.includes("code") || iconName.includes("nvim")) {
      return (
         <svg {...iconProps} viewBox="0 0 24 24" fill="#3b82f6">
            <path
               d="M14.5 2.5l-1 1 5.5 5.5-5.5 5.5 1 1 6.5-6.5-6.5-6.5zM9.5 2.5l-1 1-5.5 5.5 5.5 5.5 1 1-6.5-6.5 6.5-6.5z"
               fill="#3b82f6"
            />
         </svg>
      )
   }
   if (iconName.includes("slack")) {
      return (
         <svg {...iconProps} viewBox="0 0 24 24" fill="#4A154B">
            <path d="M5.042 15.165a2.528 2.528 0 0 1-2.52 2.523A2.528 2.528 0 0 1 0 15.165a2.527 2.527 0 0 1 2.522-2.52h2.52v2.52zM6.313 15.165a2.527 2.527 0 0 1 2.521-2.52 2.527 2.527 0 0 1 2.521 2.52v6.313A2.528 2.528 0 0 1 8.834 24a2.528 2.528 0 0 1-2.521-2.522v-6.313zM8.834 5.045a2.528 2.528 0 0 1-2.521-2.52A2.528 2.528 0 0 1 8.834 0a2.528 2.528 0 0 1 2.521 2.525v2.52H8.834zM8.834 6.313a2.528 2.528 0 0 1 2.521 2.521 2.528 2.528 0 0 1-2.521 2.521H2.522A2.528 2.528 0 0 1 0 8.834a2.528 2.528 0 0 1 2.522-2.521h6.312zM18.956 8.834a2.528 2.528 0 0 1 2.522-2.521A2.528 2.528 0 0 1 24 8.834a2.528 2.528 0 0 1-2.522 2.521h-2.522V8.834zM17.688 8.834a2.528 2.528 0 0 1-2.523 2.521 2.527 2.527 0 0 1-2.52-2.521V2.522A2.527 2.527 0 0 1 15.165 0a2.528 2.528 0 0 1 2.523 2.522v6.312zM15.165 18.956a2.528 2.528 0 0 1 2.523 2.522A2.528 2.528 0 0 1 15.165 24a2.527 2.527 0 0 1-2.52-2.522v-2.522h2.52zM15.165 17.688a2.527 2.527 0 0 1-2.52-2.523 2.526 2.526 0 0 1 2.52-2.52h6.313A2.527 2.527 0 0 1 24 15.165a2.528 2.528 0 0 1-2.522 2.523h-6.313z" />
         </svg>
      )
   }
   return (
      <Monitor
         {...iconProps}
         color={
            category === "distracting"
               ? "#ef4444"
               : category === "productive"
                 ? "#22c55e"
                 : "#fffffff"
         }
      />
   )
}

interface DropdownOption {
   value: string
   label: string
}

const dropdownOptions: DropdownOption[] = [
   { value: "today", label: "Today" },
   { value: "week", label: "This Week" },
   { value: "month", label: "This Month" },
   { value: "year", label: "This Year" },
]

function App() {
   const [dropdownOpen, setDropdownOpen] = useState(false)
   const [selectedOption, setSelectedOption] = useState(dropdownOptions[0])
   const [selectedDay, setSelectedDay] = useState("Wed")
   const [screenTimeData, setScreenTimeData] = useState<ScreenTimeData | null>(null)

   useEffect(() => {
      const fetchData = async () => {
         try {
            const data = await window.electron.getScreenTimeData()
            if (data) {
               setScreenTimeData(data)
               const today = new Date(data.today_date)
               const days = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]
               setSelectedDay(days[today.getDay()])
            }
         } catch (e) {
            console.error("Failed to fetch screen time data", e)
         }
      }
      fetchData()
      const interval = setInterval(fetchData, 5000)
      return () => clearInterval(interval)
   }, [])

   const maxHours = Math.max(
      1,
      ...(screenTimeData?.weekly_stats?.map((d: any) => d.total_seconds / 3600) || [0]),
      8
   )

   const processWeeklyData = () => {
      if (!screenTimeData?.weekly_stats) return []
      return screenTimeData.weekly_stats
         .map(stat => {
            const date = new Date(stat.date)
            const days = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]
            const label = days[date.getDay()]
            return {
               day: label,
               hours: Number((stat.total_seconds / 3600).toFixed(1)),
               label: label,
            }
         })
         .reverse()
   }

   const weeklyData = processWeeklyData()
   const apps = (screenTimeData?.app_usages || []).slice(0, 10).map((app: any) => ({
      name: app.name,
      hours: Math.floor(app.seconds / 3600),
      minutes: Math.floor((app.seconds % 3600) / 60),
      icon: app.name,
      category: app.category,
   }))

   const formatSeconds = (seconds: number) => {
      const h = Math.floor(seconds / 3600)
      const m = Math.floor((seconds % 3600) / 60)
      return `${h} hrs, ${m} mins`
   }

   const currentDateStr = screenTimeData?.today_date
      ? new Date(screenTimeData.today_date).toLocaleDateString(undefined, {
           weekday: "short",
           day: "numeric",
           month: "short",
        })
      : "Loading..."
   const yesterdayDateStr = screenTimeData?.today_date
      ? new Date(new Date(screenTimeData.today_date).getTime() - 86400000).toLocaleDateString(
           undefined,
           { weekday: "short", day: "numeric", month: "short" }
        )
      : ""
   const tomorrowDateStr = screenTimeData?.today_date
      ? new Date(new Date(screenTimeData.today_date).getTime() + 86400000).toLocaleDateString(
           undefined,
           { weekday: "short", day: "numeric", month: "short" }
        )
      : ""

   return (
      <div className="min-h-screen bg-background text-foreground font-sans selection:bg-primary/30">
         <div className="max-w-4xl mx-auto px-6 py-8 space-y-8">
            <header className="flex items-center justify-between">
               <h1 className="text-4xl font-bold tracking-tight">Dashboard</h1>

               <div className="relative">
                  <button
                     onClick={() => setDropdownOpen(!dropdownOpen)}
                     className="flex items-center gap-2 px-4 py-2.5 rounded-full bg-secondary border border-border hover:border-primary/50 transition-all duration-200 group"
                  >
                     <span className="text-sm font-medium text-muted-foreground">Screen time</span>
                     <ChevronDown
                        size={16}
                        className={`text-muted-foreground transition-transform duration-200 ${
                           dropdownOpen ? "rotate-180" : ""
                        }`}
                     />
                  </button>

                  {dropdownOpen && (
                     <div className="absolute top-full left-0 mt-2 w-full min-w-48 glass rounded-xl overflow-hidden animate-dropdown z-50">
                        {dropdownOptions.map(option => (
                           <button
                              key={option.value}
                              onClick={() => {
                                 setSelectedOption(option)
                                 setDropdownOpen(false)
                              }}
                              className="w-full px-4 py-3 text-left text-sm hover:bg-white/10 transition-colors duration-150 flex items-center justify-between group"
                           >
                              <span>{option.label}</span>
                              {selectedOption.value === option.value && (
                                 <span className="w-2 h-2 rounded-full bg-primary" />
                              )}
                           </button>
                        ))}
                     </div>
                  )}
               </div>
            </header>

            <section
               className="text-center space-y-2 animate-fade-in"
               style={{ animationDelay: "0.1s" }}
            >
               <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-primary/10 border border-primary/20">
                  <span className="w-1.5 h-1.5 rounded-full bg-primary animate-pulse" />
                  <span className="text-xs font-medium text-primary">Active</span>
               </div>
               <div className="text-7xl font-bold tracking-tight">
                  {screenTimeData ? formatSeconds(screenTimeData.today_seconds) : "0 hrs, 0 mins"}
               </div>
               <p className="text-muted-foreground text-lg">{currentDateStr}</p>
            </section>

            <section className="animate-fade-in" style={{ animationDelay: "0.2s" }}>
               <div className="glass rounded-2xl p-6">
                  <div className="flex gap-2 mb-4">
                     {weeklyData.map(item => (
                        <div
                           key={item.day}
                           className={`flex-1 flex flex-col items-center gap-3 cursor-pointer group transition-all duration-200 hover:bg-white/5 rounded-xl p-2 ${
                              selectedDay === item.label ? "bg-primary/10" : ""
                           }`}
                           onClick={() => setSelectedDay(item.label)}
                        >
                           <div className="w-full flex-1 flex flex-col justify-end min-h-40">
                              <div
                                 className={`w-full rounded-t-lg transition-all duration-500 ${
                                    selectedDay === item.label
                                       ? "bg-primary"
                                       : "bg-white/20 group-hover:bg-white/30"
                                 }`}
                                 style={{
                                    height: `${(item.hours / maxHours) * 100}%`,
                                    animationDelay: `${item.hours * 50}ms`,
                                 }}
                              />
                           </div>
                           <span
                              className={`text-xs font-medium ${
                                 selectedDay === item.label
                                    ? "text-primary"
                                    : "text-muted-foreground"
                              }`}
                           >
                              {item.day}
                           </span>
                           <span className="text-[10px] text-muted-foreground/60">
                              {item.hours}h
                           </span>
                        </div>
                     ))}
                  </div>

                  <div className="flex justify-between border-t border-border/50 pt-4 mt-4">
                     <button className="p-2 rounded-lg hover:bg-white/10 transition-colors duration-200 group">
                        <ChevronLeft
                           size={20}
                           className="text-muted-foreground group-hover:text-foreground transition-colors"
                        />
                     </button>
                     <div className="flex items-center gap-3">
                        <span className="text-sm font-medium">{yesterdayDateStr}</span>
                        <span className="text-sm text-muted-foreground">/</span>
                        <span className="text-sm font-medium text-primary">{currentDateStr}</span>
                        <span className="text-sm text-muted-foreground">/</span>
                        <span className="text-sm font-medium">{tomorrowDateStr}</span>
                     </div>
                     <button className="p-2 rounded-lg hover:bg-white/10 transition-colors duration-200 group">
                        <ChevronRight
                           size={20}
                           className="text-muted-foreground group-hover:text-foreground transition-colors"
                        />
                     </button>
                  </div>
               </div>
            </section>

            <section className="space-y-3 animate-fade-in" style={{ animationDelay: "0.3s" }}>
               <h2 className="text-lg font-semibold tracking-tight">App Usage</h2>
               <div className="space-y-2">
                  {apps.map((app, index) => (
                     <div
                        key={app.name}
                        className="group flex items-center gap-4 p-4 rounded-xl bg-card hover:bg-card/80 border border-transparent hover:border-border/50 transition-all duration-200 cursor-pointer animate-fade-in"
                        style={{ animationDelay: `${0.4 + index * 0.1}s` }}
                     >
                        <div className="w-10 h-10 rounded-xl flex items-center justify-center bg-white/10">
                           {getAppIcon(app.icon, app.category)}
                        </div>
                        <div className="flex-1 min-w-0">
                           <p className="font-semibold truncate">{app.name}</p>
                           <p className="text-sm text-muted-foreground">
                              {app.hours} hrs, {app.minutes} mins
                           </p>
                        </div>
                        <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity duration-200">
                           <button className="p-2 rounded-lg hover:bg-white/10 transition-colors duration-150">
                              <Timer size={16} className="text-muted-foreground" />
                           </button>
                           <button className="p-2 rounded-lg hover:bg-white/10 transition-colors duration-150">
                              <Focus size={16} className="text-muted-foreground" />
                           </button>
                           <button className="p-2 rounded-lg hover:bg-white/10 transition-colors duration-150">
                              <MoreHorizontal size={16} className="text-muted-foreground" />
                           </button>
                        </div>
                     </div>
                  ))}
               </div>
            </section>

            <footer className="text-center pt-8 pb-4">
               <p className="text-sm text-muted-foreground">Screen Time Tracker</p>
            </footer>
         </div>

         {dropdownOpen && (
            <div className="fixed inset-0 z-40" onClick={() => setDropdownOpen(false)} />
         )}
      </div>
   )
}

export default App
