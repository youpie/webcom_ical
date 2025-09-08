function f
    # run the finder
    python3 ~/Services/Webcom/find_user.py $argv
    
    # grab the result (if any) and cd
    set tmp /tmp/kuma-find.tmp
    if test -f $tmp
        set dest (cat $tmp)
        if test -n "$dest"
            cd $dest
            ~/Services/Webcom/logbook_check.py -s $dest
        end
    end
end
