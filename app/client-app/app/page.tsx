"use client";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { useEffect, useState } from "react";
import internal from "stream";

export default function Home() {
  const URL = "http://192.168.1.149";
  const [info, setInfo] = useState<{
    manual_mode: boolean;
    time: string;
    aspersores: {
      name: string;
      duration: number;
      init_time: number;
      on: boolean;
      pin: string;
    }[];
  }>();
  const [aspersoresUpdate, setAspersoresUpdate] = useState({});

  const getInfo = async () => {
    const res = await fetch(`${URL}/get_info`);
    const text = await res.json();
    console.log(text);

    const aspersoresData = {};
    for (const aspersor of text.aspersores) {
      const initTime = new Date();
      initTime.setTime(aspersor.init_time * 1000);

      const duration = new Date();
      duration.setTime(aspersor.duration * 1000);

      aspersoresData[aspersor.name] = {
        initTime: {
          hours: initTime.getUTCHours(),
          minutes: initTime.getMinutes(),
        },
        duration: {
          hours: duration.getUTCHours(),
          minutes: duration.getMinutes(),
        },
      };
    }

    setAspersoresUpdate(aspersoresData);
    setInfo(text);
  };

  useEffect(() => {
    getInfo();
  }, []);

  if (info) {
    return (
      <div className="grid grid-rows-[20px_1fr_20px] items-center justify-items-center min-h-screen p-8 pb-20 gap-16 sm:p-20 font-[family-name:var(--font-geist-sans)]">
        <main className="flex flex-col gap-8 row-start-2 items-center sm:items-start">
          <div style={{ marginRight: "20px" }}>{info.time}</div>
          <div className="flex flex-row items-center justify-between rounded-lg">
            <div style={{ marginRight: "20px" }}>Manual mode:</div>
            <Switch
              checked={info.manual_mode}
              onCheckedChange={async () => {
                const res = await fetch(
                  `http://192.168.1.149/toggle/manual_mode`
                );
                await res.json();
                setInfo({
                  ...info,
                  manual_mode: !info.manual_mode,
                });
              }}
            />
          </div>
          {info.aspersores.map((aspersor, i) => {
            return (
              <div
                key={aspersor.name}
                className="rounded-lg border p-3 shadow-sm"
                style={{ width: "100%" }}
              >
                <div className="flex flex-row items-center justify-between">
                  <div style={{ marginRight: "20px" }}>
                    {aspersor.name} (pin: {aspersor.pin})
                  </div>
                  <Switch
                    checked={aspersor.on}
                    onCheckedChange={async () => {
                      const res = await fetch(
                        `http://192.168.1.149/toggle/${aspersor.name}`
                      );
                      await res.json();

                      info.aspersores[i].on = !info.aspersores[i].on;
                      setInfo({
                        ...info,
                      });
                    }}
                  />
                </div>
                <div className="mt-3 flex flex-row items-center justify-between">
                  <div className="flex items-center mr-6">
                    <div className="mr-1">init time</div>
                    <Input
                      style={{ width: "45px" }}
                      placeholder="00"
                      // type="number"
                      value={aspersoresUpdate[aspersor.name]?.initTime?.hours}
                      onChange={(input) => {
                        setAspersoresUpdate({
                          ...aspersoresUpdate,
                          [aspersor.name]: {
                            ...aspersoresUpdate[aspersor.name],
                            initTime: {
                              hours: Number(input.target.value),
                              minutes:
                                aspersoresUpdate[aspersor.name]?.initTime
                                  ?.minutes,
                            },
                          },
                        });
                      }}
                    />
                    :
                    <Input
                      style={{ width: "45px" }}
                      placeholder="00"
                      // type="number"
                      value={aspersoresUpdate[aspersor.name]?.initTime?.minutes}
                      onChange={(input) => {
                        setAspersoresUpdate({
                          ...aspersoresUpdate,
                          [aspersor.name]: {
                            ...aspersoresUpdate[aspersor.name],
                            initTime: {
                              hours:
                                aspersoresUpdate[aspersor.name]?.initTime
                                  ?.hours,
                              minutes: Number(input.target.value),
                            },
                          },
                        });
                      }}
                    />
                  </div>
                  <div className="flex items-center mr-8">
                    <div className="mr-1">duration</div>
                    <Input
                      style={{ width: "45px" }}
                      placeholder="00"
                      // type="number"
                      value={aspersoresUpdate[aspersor.name]?.duration?.hours}
                      onChange={(input) => {
                        setAspersoresUpdate({
                          ...aspersoresUpdate,
                          [aspersor.name]: {
                            ...aspersoresUpdate[aspersor.name],
                            duration: {
                              hours: Number(input.target.value),
                              minutes:
                                aspersoresUpdate[aspersor.name]?.duration
                                  ?.minutes,
                            },
                          },
                        });
                      }}
                    />
                    :
                    <Input
                      style={{ width: "45px" }}
                      placeholder="00"
                      // type="number"
                      value={aspersoresUpdate[aspersor.name]?.duration?.minutes}
                      onChange={(input) => {
                        setAspersoresUpdate({
                          ...aspersoresUpdate,
                          [aspersor.name]: {
                            ...aspersoresUpdate[aspersor.name],
                            duration: {
                              hours:
                                aspersoresUpdate[aspersor.name]?.duration
                                  ?.hours,
                              minutes: Number(input.target.value),
                            },
                          },
                        });
                      }}
                    />
                  </div>

                  <Button
                    onClick={async () => {
                      console.log(aspersoresUpdate);
                      const aspersorData = aspersoresUpdate[aspersor.name];
                      const duration =
                        (aspersorData.duration.hours * 60 +
                          aspersorData.duration.minutes) *
                        60;
                      const initTime =
                        (aspersorData.initTime.hours * 60 +
                          aspersorData.initTime.minutes) *
                        60;

                      const res = await fetch(
                        `${URL}/update_aspersor/${aspersor.name}?duration=${duration}&init_time=${initTime}`
                      );
                      await res.json();

                      await getInfo();
                    }}
                  >
                    Update
                  </Button>
                </div>
              </div>
            );
          })}
        </main>
      </div>
    );
  }
}
