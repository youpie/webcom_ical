function kumaquit
    set -l all_mode 0

    # check if -a was given
    if test (count $argv) -gt 0
        if test $argv[1] = "-a"
            set all_mode 1
        end
    end

    if test $all_mode -eq 1
        # loop over all dirs in current folder
        for dir in */
            set dir (string trim -r -c '/' $dir)
            # skip dirs starting with "_"
            if string match -q "_*" $dir
                continue
            end
            if test -p "$dir/kuma/pipe"
                echo "q" > "$dir/kuma/pipe"
            end
        end
    else
        if test -p "./kuma/pipe"
            echo "q" > "./kuma/pipe"
        else
            echo "No pipe found in ./kuma/pipe"
        end
    end
end
