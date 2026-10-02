
                    local function control()
                        local c = setmetatable({hidden=false, sizeY=34}, {__index=function()
                            return function() end
                        end})
                        function c:SetHide(value) self.hidden=value end
                        function c:IsHidden() return self.hidden end
                        function c:SetText(value) self.text=value; self.sizeY=64 end
                        function c:GetSizeY() return self.sizeY end
                        function c:SetSizeY(value) self.sizeY=value end
                        function c:RegisterCallback(_, callback) self.callback=callback end
                        function c:DoLeftClick()
                            assert(not ContextPtr:IsHidden() and not self:IsHidden(), 'unreachable control')
                            self.callback()
                        end
                        return c
                    end
                    ContextPtr=control(); ContextPtr:SetHide(true)
                    Controls=setmetatable({}, {__index=function(t, key)
                        local c=control(); rawset(t, key, c); return c
                    end})
                    Controls.Panel:SetHide(true); Controls.PactPanel:SetHide(true)
                    InstanceManager={new=function(_, template, root, stack)
                        stack.instances={}
                        return {ResetInstances=function() stack.instances={} end,
                            GetInstance=function()
                                local row=setmetatable({}, {__index=function(t, key)
                                    local c=control(); rawset(t, key, c); return c
                                end})
                                table.insert(stack.instances,row)
                                return row
                            end}
                    end}
                    Events=setmetatable({}, {__index=function() return {Add=function() end} end})
                    Mouse={eLClick=1}
                    include=function() end
                    Game.GetLocalPlayer=function() return 2 end
                    Players[2].IsTurnActive=function() return true end
                    local diplomacy=Players[2].GetDiplomacy
                    Players[2].GetDiplomacy=function()
                        local d=diplomacy(); d.HasMet=function() return false end; return d
                    end
