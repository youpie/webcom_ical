function logbook
     set current $(pwd)
    cd ~/Services/Webcom/ 
    ~/Services/Webcom/logbook_check.py $argv
    cd $current
end
