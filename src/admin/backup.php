<?php
include("includes/controller.php");

$command = 'c:\xampp\mysql\bin\mysqldump --opt --host='.DB_HOST.' --user='.DB_USER.' --password='.DB_PASS .' '. DB_NAME.' > test3.sql';
exec($command);