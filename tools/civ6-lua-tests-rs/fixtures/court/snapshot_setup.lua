
            local R = ExposedMembers.JAC_CourtArbitration
            Game:SetProperty('JAC_ARB_HUMAN_TEST', {case='accept', human=2, other=0})
            Game:SetProperty(R.LAST_REQUEST_KEY, 1)
            Players[2]:SetProperty(R.LEGITIMACY_KEY, 65)
            Players[0]:SetProperty(R.LEGITIMACY_KEY, 45)
            Players[0].cities = 2
            SET_WAR(0, 2, true)
            local function engineProperties(object)
                local get = object.GetProperty
                object.GetProperty = function(self, key)
                    local value = get(self, key)
                    if value ~= nil then return value end
                end
            end
            engineProperties(Game)
            for _, id in ipairs({0, 2}) do
                engineProperties(Players[id])
                for _, city in Players[id]:GetCities():Members() do
                    local plot = Map.GetPlot(city:GetX(), city:GetY())
                    for _, key in ipairs({R.FLAG_HIGH, R.FLAG_LOW, R.FLAG_SUBJUGATION}) do
                        plot:SetProperty(key, nil)
                    end
                    engineProperties(plot)
                end
            end
