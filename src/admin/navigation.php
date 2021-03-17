<?php
include_once("includes/controller.php");
if(!$session->isAdmin()){
    header("Location: ".$configs->homePage());
    exit;
}
?>
<!-- Site Navigation -->
<ul class="nav">
    <li class="sidebar-header">IPM User</li>
    <li <?php if($pagename == 'index') { echo 'class="active selected"'; } ?>>
        <a href="index.php"><i class="oi oi-dashboard"></i> <span class="nav-label">Dashboard</span></a>
    </li>
    <?php if ($session->isSuperAdmin()){ ?>
    <li <?php if($pagename == 'configurations') { echo 'class="active selected"'; } ?>>
     
    </li>
    <li <?php if($pagename == 'registration') { echo 'class="active selected"'; } ?>>
        <a href="registration.php"><i class="oi oi-task"></i> <span class="nav-label">Registration</span></a>
    </li>
    <li <?php if($container == 'settings') { echo 'class="active selected"'; } ?>>
        <a href="#"><i class="oi oi-monitor"></i> <span class="nav-label">Settings</span> <span class="oi oi-chevron-right"></span></a>
        <ul class="nav nav-second-level">
            <li <?php if($pagename == 'session-settings') { echo 'class="active"'; } ?>><a href="session-settings.php">Session Settings</a></li>
            <li <?php if($pagename == 'user-settings') { echo 'class="active"'; } ?>><a href="user-settings.php">User Settings</a></li>
            <li <?php if($pagename == 'security') { echo 'class="active"'; } ?>><a href="security.php">Security Settings</a></li>
        </ul>
    </li>
    <?php } ?>
    <li <?php if($pagename == 'useradmin') { echo 'class="active selected"'; } ?>>
        <a href="useradmin.php"><i class="oi oi-person"></i> <span class="nav-label">User Admin</span></a>
    </li>
    <li <?php if($pagename == 'usergroups') { echo 'class="active selected"'; } ?>>
        <a href="usergroups.php"><i class="oi oi-people"></i> <span class="nav-label">User Groups</span></a>
    </li>
    <li <?php if($pagename == 'logs') { echo 'class="active selected"'; } ?>>
        <a href="logs.php"><i class="oi oi-bar-chart"></i> <span class="nav-label">Logs</span></a>
    </li>

</ul>
<!-- END Site Navigation -->
