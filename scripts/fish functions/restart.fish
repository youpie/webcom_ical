function restart
    set name (basename "$folder")

    if not test -f ./kuma/pipe
        echo "❌ Missing pipe"
        return 1
    end

    echo "e" > ./kuma/pipe
    
    while true
        sleep 3
        if not test -f "$folder/kuma/active"
            echo "Active file missing..."
        else
            break
        end
    end
    # 6) wait for logbook.json to change
    set logfile "$folder/kuma/logbook.json"
    if not test -f $logfile
        echo "❌ Missing logbook.json"
        return 1
    end

    set prev (stat -c %Y $logfile)
    while true
        sleep 1
        set now (stat -c %Y $logfile)
        if test $now -gt $prev
            break
        end
    end
    # 7) final check
    ~/Services/Webcom/logbook_check.py -s d
end