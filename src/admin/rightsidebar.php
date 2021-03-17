<!-- Right Sidebar -->
<div class="sidebar-right">
    <div id="right-sidebar-id">
        <div class="right-sidebar-header"><i class="oi oi-globe"></i>
            <span class="sidebar-header-text">Recently Online</span>
        </div>
        <div class="right-sidebar-section">
            <!-- recentlyOnline(minutes) -->
            <?php echo $adminfunctions->recentlyOnline(5); ?>
        </div>
        <div class="right-sidebar-header"><i class="oi oi-flash"></i>
            <span class="sidebar-header-text">User Activity</span>
        </div>
        <div class="right-sidebar-section">
            <?php
            $sql5 = "SELECT * FROM users WHERE username != '" . ADMIN_NAME . "' ORDER BY timestamp DESC LIMIT 1";
            $result5 = $db->prepare($sql5);
            $result5->execute();
            $row5 = $result5->fetch();
            $lastlogin = $adminfunctions->displayDate($row5['timestamp']);
            echo $row5['username']." logged on - ".$lastlogin;
            ?>
        </div>        
        <div class="right-sidebar-header"><i class="oi oi-graph"></i>
            <span class="sidebar-header-text">Statistics</span>
        </div>
        <div class="right-sidebar-section">
            <?php
            $adminactivation = $adminfunctions->displayAdminActivation('regdate');
            $num_needact = $adminactivation->rowCount();
            echo "<p><i class='oi oi-circle-check'></i> There are ".$session->getNumMembers()." members.</p>";
            echo "<p><i class='oi oi-circle-check'></i> ".$num_needact . " accounts require activation.</p>";
            echo "<p><i class='oi oi-circle-check'></i> ".$adminfunctions->usersSince($session->username) . " new users have registered since your last visit.</p>";
            echo "<p><i class='oi oi-circle-check'></i> There are currently ".$functions->calcNumActiveUsers()." users and ".$session->calcNumActiveGuests()." guests online.</p>";
            echo "<p><i class='oi oi-circle-check'></i> Record Users Online : ".$configs->getConfig('record_online_users')."</p>"; 
            ?>
        </div>
    </div>
</div>
<!-- END Right Sidebar -->
