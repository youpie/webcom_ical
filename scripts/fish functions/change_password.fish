function change_password
    set current $(pwd)
    if test (count $argv) -ne 1
        echo "Usage: change_password [name]"
        return 1
    end

    # 1) lookup user → folder path
    ~/Services/Webcom/find_user.py $argv[1]
    set folder $(cat /tmp/kuma-find.tmp)
    echo "Working in folder $folder"    
    # 2) stop if empty
    if test -z "$folder"
        echo "❌ No user/folder found."
        return 1
    end

    # 3) prompt for new password
    read -s -P "New password: " pass1; echo

    # 4) set in env
    ~/Services/Webcom/env_manager.sh --dirs "$folder" --set PASSWORD="$pass1"

    # 5) restart the ical container
    set name (basename "$folder")

    if not test -f $folder/kuma/pipe
        echo "❌ Missing pipe"
        return 1
    end

    # docker start webcom_$name-webcom_ical-1
    echo "e" > $folder/kuma/pipe


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
    cd $folder
    # 7) final check
    ~/Services/Webcom/logbook_check.py -s
    cd $current
end
